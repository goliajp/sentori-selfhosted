//! Reviewed console mutations: project authorization, version fences, receipts and audit.
pub(super) use crate::handlers::jobs::database_error;
use crate::{session_mw::SessionContext, state::AppState};
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Row, Transaction};
use std::sync::Arc;
use uuid::Uuid;
pub type Error = (StatusCode, Json<Value>);
pub fn error(status: StatusCode, code: &str) -> Error {
    (status, Json(json!({"error":code})))
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Command {
    pub command_id: String,
    pub action: Action,
}
#[derive(Deserialize, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Action {
    CreateCredential {
        provider: String,
        config: Value,
        secret: String,
        label: String,
    },
    ActivateCredential {
        id: Uuid,
        expected_version: Option<i64>,
        #[serde(default)]
        force: bool,
    },
    DeleteCredential {
        id: Uuid,
        expected_version: Option<i64>,
    },
    TestSend {
        device_id: Uuid,
        title: String,
        body: String,
    },
    RetrySend {
        id: Uuid,
        expected_version: Option<i64>,
        #[serde(default)]
        acknowledge_duplicate: bool,
    },
    RetryFailed {
        ids: Vec<Uuid>,
    },
    RevokeDevice {
        id: Uuid,
        expected_version: Option<i64>,
    },
}
pub fn digest(value: &impl Serialize) -> Result<String, Error> {
    Ok(hex::encode(Sha256::digest(
        serde_json::to_vec(value).map_err(|_| error(StatusCode::BAD_REQUEST, "invalid_command"))?,
    )))
}
pub async fn lock(tx: &mut Transaction<'_, Postgres>, project: Uuid) -> Result<(), Error> {
    sqlx::query("SELECT id FROM projects WHERE id=$1 FOR NO KEY UPDATE")
        .bind(project)
        .fetch_optional(&mut **tx)
        .await
        .map_err(database_error)?
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "project_not_found"))?;
    Ok(())
}
pub async fn prior(
    tx: &mut Transaction<'_, Postgres>,
    project: Uuid,
    actor: Uuid,
    key: &str,
    digest: &str,
) -> Result<Option<Value>, Error> {
    if key.is_empty() || key.len() > 128 || !key.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(error(StatusCode::BAD_REQUEST, "invalid_command_id"));
    }
    let row=sqlx::query("SELECT digest,response FROM push_commands WHERE project_id=$1 AND actor_user_id=$2 AND command_id=$3").bind(project).bind(actor).bind(key).fetch_optional(&mut **tx).await.map_err(database_error)?;
    row.map(|row| {
        if row.get::<String, _>("digest") == digest {
            Ok(row.get("response"))
        } else {
            Err(error(StatusCode::CONFLICT, "idempotency_conflict"))
        }
    })
    .transpose()
}
pub async fn record(
    tx: &mut Transaction<'_, Postgres>,
    project: Uuid,
    actor: Uuid,
    key: &str,
    digest: &str,
    response: &Value,
) -> Result<(), Error> {
    sqlx::query("INSERT INTO push_commands(project_id,actor_user_id,command_id,digest,response) VALUES($1,$2,$3,$4,$5)").bind(project).bind(actor).bind(key).bind(digest).bind(response).execute(&mut **tx).await.map_err(database_error)?;
    Ok(())
}
pub fn validate_message(title: &str, body: &str) -> Result<(), Error> {
    if title.trim().is_empty()
        || title.chars().count() > 120
        || serde_json::to_vec(&json!({"title":title,"body":body})).map_or(true, |v| v.len() > 3500)
    {
        return Err(error(StatusCode::BAD_REQUEST, "invalid_push_message"));
    }
    Ok(())
}
pub fn config_keys(provider: &str) -> &'static [&'static str] {
    match provider {
        "apns" => &["keyId", "teamId", "topic", "production"],
        "fcm" => &["fcmProjectId"],
        "webpush" => &["subject", "vapidPublicKey"],
        "hcm" => &["clientId", "appId"],
        "mipush" => &["packageName"],
        _ => &[],
    }
}
fn version(actual: i64, expected: Option<i64>) -> Result<(), Error> {
    if expected.is_some_and(|expected| expected != actual) {
        return Err(error(StatusCode::CONFLICT, "push_changed"));
    }
    Ok(())
}
pub async fn handle(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project): Path<Uuid>,
    Json(command): Json<Command>,
) -> Result<Json<Value>, Error> {
    super::push_commands::execute(&state, &ctx, project, command)
        .await
        .map(Json)
}
pub async fn execute(
    state: &Arc<AppState>,
    ctx: &SessionContext,
    project: Uuid,
    command: Command,
) -> Result<Value, Error> {
    super::tokens::ensure_project_access(state, ctx, project).await?;
    let hash = digest(&command.action)?;
    let mut tx = state.pool.begin().await.map_err(database_error)?;
    lock(&mut tx, project).await?;
    if let Some(value) = prior(&mut tx, project, ctx.user_id, &command.command_id, &hash).await? {
        return Ok(value);
    }
    let (response, audit, target) = match command.action {
        Action::CreateCredential {
            provider,
            config,
            secret,
            label,
        } => create_credential(state, &mut tx, project, provider, config, secret, label).await?,
        Action::ActivateCredential {
            id,
            expected_version,
            force,
        } => activate_credential(&mut tx, project, id, expected_version, force).await?,
        Action::DeleteCredential {
            id,
            expected_version,
        } => delete_credential(&mut tx, project, id, expected_version).await?,
        Action::TestSend {
            device_id,
            title,
            body,
        } => test_send(&mut tx, project, device_id, title, body).await?,
        Action::RetrySend {
            id,
            expected_version,
            acknowledge_duplicate,
        } => {
            retry_send(
                &mut tx,
                project,
                id,
                expected_version,
                acknowledge_duplicate,
            )
            .await?
        }
        Action::RetryFailed { ids } => retry_failed(&mut tx, project, ids).await?,
        Action::RevokeDevice {
            id,
            expected_version,
        } => revoke_device(&mut tx, project, id, expected_version).await?,
    };
    crate::audit::record_in_transaction(
        &mut tx,
        Some(project),
        ctx.user_id,
        audit,
        "push",
        &target.to_string(),
        response.clone(),
    )
    .await
    .map_err(database_error)?;
    record(
        &mut tx,
        project,
        ctx.user_id,
        &command.command_id,
        &hash,
        &response,
    )
    .await?;
    tx.commit().await.map_err(database_error)?;
    Ok(response)
}
async fn requeue(
    tx: &mut Transaction<'_, Postgres>,
    project: Uuid,
    ids: &[Uuid],
) -> Result<(), Error> {
    sqlx::query("UPDATE push_sends SET status='queued',next_attempt_at=now(),retry_count=0,error=NULL,provider_outcome=NULL,lease_owner=NULL,lease_until=NULL,credential_id=NULL WHERE project_id=$1 AND id=ANY($2)").bind(project).bind(ids).execute(&mut **tx).await.map_err(database_error)?;
    Ok(())
}

async fn create_credential(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    project: Uuid,
    provider: String,
    mut config: Value,
    secret: String,
    label: String,
) -> Result<(Value, &'static str, Uuid), Error> {
    let keys = config_keys(&provider);
    if keys.is_empty()
        || !config.is_object()
        || secret.is_empty()
        || secret.len() > 65536
        || label.chars().count() > 100
        || config.to_string().len() > 8192
    {
        return Err(error(StatusCode::BAD_REQUEST, "invalid_credential"));
    }
    crate::push_credential_check::check(&provider,&mut config,&secret).map_err(|r|(StatusCode::BAD_REQUEST,Json(json!({"error":"invalid_credential","code":r.code,"field":r.field,"detail":r.detail}))))?;
    if config
        .as_object()
        .is_some_and(|v| v.keys().any(|k| !keys.contains(&k.as_str())))
    {
        return Err(error(StatusCode::BAD_REQUEST, "invalid_credential"));
    }
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM push_credentials WHERE project_id=$1")
            .bind(project)
            .fetch_one(&mut **tx)
            .await
            .map_err(database_error)?;
    if count >= 32 {
        return Err(error(StatusCode::CONFLICT, "credential_limit"));
    }
    let id = Uuid::now_v7();
    let vault = state
        .vault
        .as_deref()
        .ok_or_else(|| error(StatusCode::SERVICE_UNAVAILABLE, "vault_unconfigured"))?;
    let sealed = crate::push_secrets::seal(vault, project, id, &provider, &secret)
        .map_err(|code| error(StatusCode::INTERNAL_SERVER_ERROR, code))?;
    sqlx::query("INSERT INTO push_credentials(id,project_id,kind,config,secret_blob,secret_format,active,label) VALUES($1,$2,$3,$4,$5,'sealed',false,$6)").bind(id).bind(project).bind(provider).bind(config).bind(sealed).bind(label.trim()).execute(&mut **tx).await.map_err(database_error)?;
    Ok((
        json!({"id":id,"active":false,"version":1}),
        "push_credentials.create",
        id,
    ))
}

async fn activate_credential(
    tx: &mut Transaction<'_, Postgres>,
    project: Uuid,
    id: Uuid,
    expected_version: Option<i64>,
    force: bool,
) -> Result<(Value, &'static str, Uuid), Error> {
    let row=sqlx::query("SELECT kind,version,last_validate_status FROM push_credentials WHERE id=$1 AND project_id=$2 FOR UPDATE").bind(id).bind(project).fetch_optional(&mut **tx).await.map_err(database_error)?.ok_or_else(||error(StatusCode::NOT_FOUND,"credential_not_found"))?;
    version(row.get("version"), expected_version)?;
    let verdict: Option<String> = row.get("last_validate_status");
    if verdict.as_deref() != Some("ok") && !force {
        return Err(error(StatusCode::CONFLICT, "credential_unverified"));
    }
    sqlx::query("UPDATE push_credentials SET active=false WHERE project_id=$1 AND kind=$2 AND active AND id<>$3").bind(project).bind(row.get::<String,_>("kind")).bind(id).execute(&mut **tx).await.map_err(database_error)?;
    sqlx::query("UPDATE push_credentials SET active=true WHERE id=$1")
        .bind(id)
        .execute(&mut **tx)
        .await
        .map_err(database_error)?;
    Ok((
        json!({"id":id,"ok":true,"forced":force,"verdict":verdict}),
        "push_credentials.activate",
        id,
    ))
}

async fn delete_credential(
    tx: &mut Transaction<'_, Postgres>,
    project: Uuid,
    id: Uuid,
    expected_version: Option<i64>,
) -> Result<(Value, &'static str, Uuid), Error> {
    let row = sqlx::query(
        "SELECT version FROM push_credentials WHERE id=$1 AND project_id=$2 FOR UPDATE",
    )
    .bind(id)
    .bind(project)
    .fetch_optional(&mut **tx)
    .await
    .map_err(database_error)?
    .ok_or_else(|| error(StatusCode::NOT_FOUND, "credential_not_found"))?;
    version(row.get("version"), expected_version)?;
    sqlx::query("DELETE FROM push_credentials WHERE id=$1")
        .bind(id)
        .execute(&mut **tx)
        .await
        .map_err(database_error)?;
    Ok((json!({"id":id,"ok":true}), "push_credentials.delete", id))
}

async fn test_send(
    tx: &mut Transaction<'_, Postgres>,
    project: Uuid,
    device_id: Uuid,
    title: String,
    body: String,
) -> Result<(Value, &'static str, Uuid), Error> {
    validate_message(&title, &body)?;
    let row=sqlx::query("SELECT provider FROM device_tokens WHERE id=$1 AND project_id=$2 AND revoked_at IS NULL FOR UPDATE").bind(device_id).bind(project).fetch_optional(&mut **tx).await.map_err(database_error)?.ok_or_else(||error(StatusCode::NOT_FOUND,"device_token_not_found_or_revoked"))?;
    let provider: String = row.get("provider");
    let credential: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM push_credentials WHERE project_id=$1 AND kind=$2 AND active",
    )
    .bind(project)
    .bind(&provider)
    .fetch_optional(&mut **tx)
    .await
    .map_err(database_error)?;
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO push_sends(id,project_id,token_id,provider,payload,status,credential_id) VALUES($1,$2,$3,$4,$5,'queued',$6)").bind(id).bind(project).bind(device_id).bind(provider).bind(json!({"title":title,"body":body})).bind(credential).execute(&mut **tx).await.map_err(database_error)?;
    Ok((
        json!({"sendId":id,"send_id":id,"status":"queued"}),
        "push.test",
        id,
    ))
}

async fn retry_send(
    tx: &mut Transaction<'_, Postgres>,
    project: Uuid,
    id: Uuid,
    expected_version: Option<i64>,
    acknowledge_duplicate: bool,
) -> Result<(Value, &'static str, Uuid), Error> {
    let row=sqlx::query("SELECT version,status,provider_outcome,generation FROM push_sends WHERE id=$1 AND project_id=$2 FOR UPDATE").bind(id).bind(project).fetch_optional(&mut **tx).await.map_err(database_error)?.ok_or_else(||error(StatusCode::NOT_FOUND,"send_not_found"))?;
    version(row.get("version"), expected_version)?;
    if row.get::<String, _>("status") != "failed" {
        return Err(error(StatusCode::CONFLICT, "not_failed"));
    }
    if row.get::<Option<String>, _>("provider_outcome").as_deref() == Some("uncertain")
        && !acknowledge_duplicate
    {
        return Err(error(
            StatusCode::CONFLICT,
            "duplicate_confirmation_required",
        ));
    }
    let current: i64 =
        sqlx::query_scalar("SELECT generation FROM instance_settings WHERE singleton")
            .fetch_one(&mut **tx)
            .await
            .map_err(database_error)?;
    if current != row.get::<i64, _>("generation") {
        return Err(error(StatusCode::CONFLICT, "instance_changed"));
    }
    requeue(tx, project, &[id]).await?;
    Ok((json!({"id":id,"status":"queued"}), "push.retry", id))
}

async fn retry_failed(
    tx: &mut Transaction<'_, Postgres>,
    project: Uuid,
    mut ids: Vec<Uuid>,
) -> Result<(Value, &'static str, Uuid), Error> {
    if ids.is_empty() || ids.len() > 1000 {
        return Err(error(StatusCode::BAD_REQUEST, "invalid_retry_selection"));
    }
    ids.sort();
    ids.dedup();
    let rows:Vec<Uuid>=sqlx::query_scalar("SELECT p.id FROM push_sends p JOIN instance_settings s ON s.singleton AND s.generation=p.generation WHERE p.project_id=$1 AND p.id=ANY($2) AND p.status='failed' AND p.provider_outcome IS DISTINCT FROM 'uncertain' ORDER BY p.id FOR UPDATE OF p").bind(project).bind(&ids).fetch_all(&mut **tx).await.map_err(database_error)?;
    if rows != ids {
        return Err(error(StatusCode::CONFLICT, "push_changed"));
    }
    requeue(tx, project, &ids).await?;
    Ok((
        json!({"requeued":ids.len()}),
        "push.retry_selected",
        project,
    ))
}

async fn revoke_device(
    tx: &mut Transaction<'_, Postgres>,
    project: Uuid,
    id: Uuid,
    expected_version: Option<i64>,
) -> Result<(Value, &'static str, Uuid), Error> {
    let row =
        sqlx::query("SELECT version FROM device_tokens WHERE id=$1 AND project_id=$2 FOR UPDATE")
            .bind(id)
            .bind(project)
            .fetch_optional(&mut **tx)
            .await
            .map_err(database_error)?
            .ok_or_else(|| error(StatusCode::NOT_FOUND, "device_not_found"))?;
    version(row.get("version"), expected_version)?;
    sqlx::query("UPDATE device_tokens SET revoked_at=COALESCE(revoked_at,now()),updated_at=now() WHERE id=$1").bind(id).execute(&mut **tx).await.map_err(database_error)?;
    Ok((
        json!({"id":id,"status":"revoked"}),
        "push.device_revoke",
        id,
    ))
}
