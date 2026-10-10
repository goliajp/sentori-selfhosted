//! Signed generic webhook adapter. This does not claim a live GitHub/Jira connection.
use super::jobs::database_error;
use crate::{session_mw::SessionContext, state::AppState};
use axum::{
    Extension, Json,
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::sync::Arc;
use time::OffsetDateTime;
use uuid::Uuid;
type Error = (StatusCode, Json<Value>);
fn error(status: StatusCode, code: &str) -> Error {
    (status, Json(json!({"error":code})))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Configure {
    repository: String,
    environment: String,
    enabled: bool,
    rotate_secret: bool,
    expected_version: i64,
}
pub async fn get(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project): Path<Uuid>,
) -> Result<Json<Value>, Error> {
    super::admin::tokens::ensure_project_access(&state, &ctx, project).await?;
    let config:Option<Value>=sqlx::query_scalar("SELECT jsonb_build_object('repository',repository,'environment',environment,'enabled',enabled,'version',version,'secretConfigured',true) FROM task_integrations WHERE project_id=$1").bind(project).fetch_optional(&state.pool).await.map_err(database_error)?;
    Ok(Json(
        json!({"vaultConfigured":state.vault.is_some(),"configuration":config}),
    ))
}
pub async fn configure(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project): Path<Uuid>,
    Json(body): Json<Configure>,
) -> Result<Json<Value>, Error> {
    if !ctx.role.is_superadmin() {
        return Err(error(StatusCode::FORBIDDEN, "owner_required"));
    }
    let vault = state
        .vault
        .as_ref()
        .ok_or_else(|| error(StatusCode::SERVICE_UNAVAILABLE, "vault_unconfigured"))?;
    if body.repository.is_empty()
        || body.repository.len() > 200
        || !body
            .repository
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/._-".contains(&b))
        || body.environment.is_empty()
        || body.environment.len() > 100
        || !body
            .environment
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    {
        return Err(error(StatusCode::BAD_REQUEST, "invalid_integration"));
    }
    let mut tx = state.pool.begin().await.map_err(database_error)?;
    sqlx::query("SELECT id FROM projects WHERE id=$1 FOR NO KEY UPDATE")
        .bind(project)
        .fetch_optional(&mut *tx)
        .await
        .map_err(database_error)?
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "not_found"))?;
    let current = sqlx::query(
        "SELECT version,secret_blob FROM task_integrations WHERE project_id=$1 FOR UPDATE",
    )
    .bind(project)
    .fetch_optional(&mut *tx)
    .await
    .map_err(database_error)?;
    let version = current.as_ref().map_or(0, |r| r.get::<i64, _>("version"));
    if body.expected_version != version {
        return Err(error(StatusCode::CONFLICT, "integration_changed"));
    }
    let secret =
        (current.is_none() || body.rotate_secret).then(|| hex::encode(rand::random::<[u8; 32]>()));
    let sealed = if let Some(secret) = &secret {
        vault
            .seal(format!("{project}:{secret}").as_bytes())
            .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "encryption_failed"))?
    } else {
        current
            .as_ref()
            .map(|r| r.get::<Vec<u8>, _>("secret_blob"))
            .ok_or_else(|| error(StatusCode::INTERNAL_SERVER_ERROR, "internal"))?
    };
    sqlx::query("INSERT INTO task_integrations(project_id,repository,environment,enabled,secret_blob) VALUES($1,$2,$3,$4,$5) ON CONFLICT(project_id) DO UPDATE SET repository=$2,environment=$3,enabled=$4,secret_blob=$5,version=task_integrations.version+1,updated_at=now()")
        .bind(project).bind(body.repository).bind(body.environment).bind(body.enabled).bind(sealed).execute(&mut *tx).await.map_err(database_error)?;
    crate::audit::record_in_transaction(
        &mut tx,
        Some(project),
        ctx.user_id,
        "task.integration_updated",
        "project",
        &project.to_string(),
        json!({"version":version+1,"rotated":secret.is_some()}),
    )
    .await
    .map_err(database_error)?;
    tx.commit().await.map_err(database_error)?;
    Ok(Json(json!({"version":version+1,"secret":secret})))
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Signal {
    delivery_id: String,
    task_id: Uuid,
    repository: String,
    commit: String,
    kind: String,
    #[serde(with = "time::serde::rfc3339")]
    occurred_at: OffsetDateTime,
    number: Option<i64>,
    unit: Option<String>,
    e2e: Option<String>,
    release: Option<String>,
    environment: Option<String>,
}
impl Signal {
    fn valid(&self) -> bool {
        !self.delivery_id.is_empty()
            && self.delivery_id.len() <= 128
            && self.delivery_id.bytes().all(|b| b.is_ascii_graphic())
            && matches!(self.commit.len(), 40 | 64)
            && self.commit.bytes().all(|b| b.is_ascii_hexdigit())
            && self.occurred_at <= OffsetDateTime::now_utc()
            && match self.kind.as_str() {
                "work.started" => true,
                "pr.opened" | "pr.merged" => self.number.is_some_and(|n| n > 0),
                "ci.completed" => {
                    self.unit
                        .as_deref()
                        .is_some_and(|v| matches!(v, "passed" | "failed"))
                        && self
                            .e2e
                            .as_deref()
                            .is_some_and(|v| matches!(v, "passed" | "failed"))
                }
                "release.deployed" => {
                    self.release
                        .as_ref()
                        .is_some_and(|v| !v.trim().is_empty() && v.len() <= 200)
                        && self.environment.is_some()
                }
                _ => false,
            }
    }
}
pub async fn receive(
    State(state): State<Arc<AppState>>,
    Path(project): Path<Uuid>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, Error> {
    if body.len() > 16384 {
        return Err(error(StatusCode::PAYLOAD_TOO_LARGE, "signal_too_large"));
    }
    let vault = state
        .vault
        .as_ref()
        .ok_or_else(|| error(StatusCode::SERVICE_UNAVAILABLE, "vault_unconfigured"))?;
    // Serialize configuration and command processing, then lock issue before task.
    let mut tx = state.pool.begin().await.map_err(database_error)?;
    sqlx::query("SELECT id FROM projects WHERE id=$1 FOR NO KEY UPDATE")
        .bind(project)
        .fetch_optional(&mut *tx)
        .await
        .map_err(database_error)?
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "not_found"))?;
    let config = sqlx::query("SELECT * FROM task_integrations WHERE project_id=$1 AND enabled")
        .bind(project)
        .fetch_optional(&mut *tx)
        .await
        .map_err(database_error)?
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "integration_unavailable"))?;
    verify_signature(vault, &config, project, &headers, &body)?;
    let mut signal: Signal = serde_json::from_slice(&body)
        .map_err(|_| error(StatusCode::BAD_REQUEST, "invalid_signal"))?;
    if !signal.valid()
        || signal.repository != config.get::<String, _>("repository")
        || signal.kind == "release.deployed"
            && signal.environment.as_deref()
                != Some(config.get::<String, _>("environment").as_str())
    {
        return Err(error(StatusCode::BAD_REQUEST, "signal_scope_mismatch"));
    }
    signal.commit.make_ascii_lowercase();
    let digest = hex::encode(Sha256::digest(&body));
    if let Some(prior) = sqlx::query(
        "SELECT digest FROM task_signal_deliveries WHERE project_id=$1 AND delivery_id=$2",
    )
    .bind(project)
    .bind(&signal.delivery_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(database_error)?
    {
        if prior.get::<String, _>("digest") != digest {
            return Err(error(StatusCode::CONFLICT, "delivery_conflict"));
        }
        return Ok(Json(json!({"outcome":"duplicate"})));
    }
    let issue: Option<Uuid> =
        sqlx::query_scalar("SELECT issue_id FROM tasks WHERE project_id=$1 AND id=$2")
            .bind(project)
            .bind(signal.task_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(database_error)?
            .ok_or_else(|| error(StatusCode::NOT_FOUND, "not_found"))?;
    if let Some(issue) = issue {
        sqlx::query("SELECT id FROM issues WHERE id=$1 FOR UPDATE")
            .bind(issue)
            .fetch_optional(&mut *tx)
            .await
            .map_err(database_error)?;
    }
    let task = sqlx::query("SELECT * FROM tasks WHERE project_id=$1 AND id=$2 FOR UPDATE")
        .bind(project)
        .bind(signal.task_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(database_error)?;
    let version: i64 = config.get("version");
    let previous: Value = task.get("automation");
    let barrier = previous
        .get("regressionAt")
        .and_then(Value::as_str)
        .and_then(|s| OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339).ok())
        .unwrap_or(task.get("created_at"));
    let ignored = signal.occurred_at <= barrier || task.get::<String, _>("stage") == "cancelled";
    let delivery = Uuid::now_v7();
    let payload = serde_json::to_value(&signal)
        .map_err(|_| error(StatusCode::BAD_REQUEST, "invalid_signal"))?;
    sqlx::query("INSERT INTO task_signal_deliveries(id,project_id,task_id,delivery_id,configuration_version,repository,commit_sha,environment,kind,occurred_at,signature_verified,digest,payload,outcome) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,true,$11,$12,$13)")
        .bind(delivery).bind(project).bind(signal.task_id).bind(&signal.delivery_id).bind(version).bind(&signal.repository).bind(&signal.commit).bind(&signal.environment).bind(&signal.kind).bind(signal.occurred_at).bind(digest).bind(payload).bind(if ignored{"ignored"}else{"accepted"}).execute(&mut *tx).await.map_err(database_error)?;
    if !ignored {
        let snapshot = crate::team::TaskSnapshot {
            stage: task.get("stage"),
            assignee: task.get("assignee_user_id"),
        };
        apply_evidence(
            &mut tx, project, &signal, version, barrier, &snapshot, &previous, issue,
        )
        .await?;
    }
    tx.commit().await.map_err(database_error)?;
    Ok(Json(
        json!({"outcome":if ignored{"ignored"}else{"accepted"}}),
    ))
}

#[allow(
    clippy::too_many_arguments,
    reason = "Already validated identifiers and locked task snapshot share one transaction"
)]
async fn apply_evidence(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    project: Uuid,
    signal: &Signal,
    version: i64,
    barrier: OffsetDateTime,
    task: &crate::team::TaskSnapshot,
    previous: &Value,
    issue: Option<Uuid>,
) -> Result<(), Error> {
    let authority:Option<Value>=sqlx::query_scalar("SELECT payload FROM task_signal_deliveries WHERE task_id=$1 AND configuration_version=$2 AND occurred_at>$3 AND outcome='accepted' AND kind IN ('work.started','pr.opened','pr.merged') ORDER BY occurred_at DESC,id DESC LIMIT 1").bind(signal.task_id).bind(version).bind(barrier).fetch_optional(&mut **tx).await.map_err(database_error)?;
    if let Some(authority) = authority {
        let commit = authority["commit"]
            .as_str()
            .ok_or_else(|| error(StatusCode::INTERNAL_SERVER_ERROR, "internal"))?;
        let proofs:Vec<(String,Value)>=sqlx::query_as("SELECT DISTINCT ON(kind) kind,payload FROM task_signal_deliveries WHERE task_id=$1 AND configuration_version=$2 AND occurred_at>$3 AND commit_sha=$4 AND outcome='accepted' AND kind IN ('ci.completed','release.deployed') ORDER BY kind,occurred_at DESC,id DESC").bind(signal.task_id).bind(version).bind(barrier).bind(commit).fetch_all(&mut **tx).await.map_err(database_error)?;
        let checks = proofs
            .iter()
            .find(|(kind, _)| kind == "ci.completed")
            .map(|(_, v)| v);
        let deployment = proofs
            .iter()
            .find(|(kind, _)| kind == "release.deployed")
            .map(|(_, v)| v);
        let passed = authority["kind"] == "pr.merged"
            && checks.is_some_and(|v| v["unit"] == "passed" && v["e2e"] == "passed");
        let deployed_after_merge = deployment.is_some_and(|d| {
            let parse = |value: &Value| {
                value.as_str().and_then(|s| {
                    OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339).ok()
                })
            };
            parse(&d["occurredAt"])
                .zip(parse(&authority["occurredAt"]))
                .is_some_and(|(deployed, merged)| deployed >= merged)
        });
        let verified = passed && deployed_after_merge;
        let next_stage = if verified {
            "done"
        } else if authority["kind"] == "pr.merged" {
            "verify"
        } else if authority["kind"] == "pr.opened" {
            "review"
        } else {
            "inProgress"
        };
        let release = if verified {
            deployment.and_then(|v| v["release"].as_str()).unwrap_or("")
        } else {
            ""
        };
        let evidence = json!({"regressionAt":crate::wire_time::rfc3339(barrier),"configurationVersion":version,"workingCommit":commit,"authority":authority,"checks":checks,"deployment":deployment});
        // Late evidence for another commit is recorded but cannot rewrite the active task.
        if &evidence != previous || task.stage != next_stage {
            sqlx::query("UPDATE tasks SET stage=$2,verified=$3,fix_release=$4,automation=$5,version=version+1,updated_at=now() WHERE id=$1").bind(signal.task_id).bind(next_stage).bind(verified).bind(release).bind(evidence).execute(&mut **tx).await.map_err(database_error)?;
            crate::team::event(
                tx,
                signal.task_id,
                None,
                &format!("auto.{}", signal.kind),
                commit,
                task.assignee,
                "task",
            )
            .await
            .map_err(database_error)?;
            sqlx::query("INSERT INTO audit_logs(id,project_id,action,target_type,target_id,payload) VALUES($1,$2,'task.automated','task',$3,$4)").bind(Uuid::now_v7()).bind(project).bind(signal.task_id.to_string()).bind(json!({"deliveryId":signal.delivery_id,"stage":next_stage,"commit":commit})).execute(&mut **tx).await.map_err(database_error)?;
            if let Some(issue) = issue {
                if !verified && task.stage == "done" {
                    sqlx::query("UPDATE issues SET status='open',resolved_at=NULL,resolved_in_release=NULL WHERE id=$1 AND status='resolved'").bind(issue).execute(&mut **tx).await.map_err(database_error)?;
                    sqlx::query("INSERT INTO issue_activity(id,issue_id,kind,body) VALUES($1,$2,'status',$3)").bind(Uuid::now_v7()).bind(issue).bind(json!({"taskId":signal.task_id,"commit":commit,"verified":false})).execute(&mut **tx).await.map_err(database_error)?;
                }
                if verified {
                    sqlx::query("UPDATE issues SET status='resolved',resolved_in_release=$2,resolved_at=now(),regressed_at=NULL,regressed_in_release=NULL WHERE id=$1").bind(issue).bind(release).execute(&mut **tx).await.map_err(database_error)?;
                    sqlx::query("INSERT INTO issue_activity(id,issue_id,kind,body) VALUES($1,$2,'status',$3)").bind(Uuid::now_v7()).bind(issue).bind(json!({"taskId":signal.task_id,"commit":commit,"release":release,"verified":true})).execute(&mut **tx).await.map_err(database_error)?;
                }
            }
        }
    }
    Ok(())
}

fn verify_signature(
    vault: &sentori_secrets_vault::Vault,
    config: &sqlx::postgres::PgRow,
    project: Uuid,
    headers: &HeaderMap,
    body: &[u8],
) -> Result<(), Error> {
    let timestamp = headers
        .get("x-sentori-timestamp")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| error(StatusCode::UNAUTHORIZED, "invalid_signature"))?;
    let at = timestamp
        .parse::<i64>()
        .map_err(|_| error(StatusCode::UNAUTHORIZED, "invalid_signature"))?;
    if OffsetDateTime::now_utc().unix_timestamp().abs_diff(at) > 300 {
        return Err(error(StatusCode::UNAUTHORIZED, "expired_signature"));
    }
    let signature = headers
        .get("x-sentori-signature")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("sha256="))
        .and_then(|v| hex::decode(v).ok())
        .ok_or_else(|| error(StatusCode::UNAUTHORIZED, "invalid_signature"))?;
    let plaintext = vault
        .open(&config.get::<Vec<u8>, _>("secret_blob"))
        .map_err(|_| error(StatusCode::SERVICE_UNAVAILABLE, "decryption_failed"))?;
    let plaintext = std::str::from_utf8(&plaintext)
        .map_err(|_| error(StatusCode::SERVICE_UNAVAILABLE, "decryption_failed"))?;
    let key = plaintext
        .strip_prefix(&format!("{project}:"))
        .ok_or_else(|| error(StatusCode::SERVICE_UNAVAILABLE, "decryption_failed"))?;
    let mut mac = Hmac::<Sha256>::new_from_slice(key.as_bytes())
        .map_err(|_| error(StatusCode::UNAUTHORIZED, "invalid_signature"))?;
    mac.update(timestamp.as_bytes());
    mac.update(b".");
    mac.update(body);
    mac.verify_slice(&signature)
        .map_err(|_| error(StatusCode::UNAUTHORIZED, "invalid_signature"))?;
    Ok(())
}
