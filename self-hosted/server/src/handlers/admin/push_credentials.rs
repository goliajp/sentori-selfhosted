//! Staged, encrypted provider credentials and explicit activation after a real probe.
use super::push_commands::{self, Action, Command, Error, database_error, error};
use crate::{push_credential_probe::Verdict, session_mw::SessionContext, state::AppState};
use axum::{
    Json,
    extract::{Extension, Path, Query, State},
    http::StatusCode,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBody {
    pub provider: String,
    pub config: Value,
    pub secret: Option<String>,
    pub label: Option<String>,
    pub command_id: Option<String>,
}
pub async fn create(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project): Path<Uuid>,
    Json(body): Json<CreateBody>,
) -> Result<(StatusCode, Json<Value>), Error> {
    let value = push_commands::execute(
        &state,
        &ctx,
        project,
        Command {
            command_id: body
                .command_id
                .unwrap_or_else(|| Uuid::now_v7().to_string()),
            action: Action::CreateCredential {
                provider: body.provider,
                config: body.config,
                secret: body.secret.unwrap_or_default(),
                label: body.label.unwrap_or_default(),
            },
        },
    )
    .await?;
    Ok((StatusCode::CREATED, Json(value)))
}
pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project): Path<Uuid>,
) -> Result<Json<Value>, Error> {
    super::tokens::ensure_project_access(&state, &ctx, project).await?;
    let rows=sqlx::query("SELECT id,kind,config,secret_blob,secret_format,version,created_at,active,label,last_validated_at,last_validate_status,last_validate_detail FROM push_credentials WHERE project_id=$1 ORDER BY kind,active DESC,created_at DESC,id DESC").bind(project).fetch_all(&state.pool).await.map_err(database_error)?;
    let credentials:Vec<Value>=rows.iter().map(|row|{
        let id:Uuid=row.get("id");let kind:String=row.get("kind");let mut config:Value=row.get("config");
        if let Some(config)=config.as_object_mut(){config.retain(|key,_|push_commands::config_keys(&kind).contains(&key.as_str()));}
        let secret=crate::push_secrets::open(state.vault.as_deref(),project,id,&kind,&row.get::<String,_>("secret_format"),&row.get::<Vec<u8>,_>("secret_blob"));
        let problem=match secret {
            Ok(secret)=>crate::push_credential_check::check(&kind,&mut config,&secret).err().map(|r|json!({"code":r.code,"field":r.field})),
            Err(code)=>Some(json!({"code":code,"field":"secret"})),
        };
        json!({"id":id,"kind":kind,"config":config,"version":row.get::<i64,_>("version"),"label":row.get::<Option<String>,_>("label"),"active":row.get::<bool,_>("active"),"problem":problem,"encrypted":row.get::<String,_>("secret_format")=="sealed","created_at":crate::wire_time::rfc3339(row.get("created_at")),"last_validated_at":crate::wire_time::rfc3339_opt(row.get("last_validated_at")),"last_validate_status":row.get::<Option<String>,_>("last_validate_status"),"last_validate_detail":row.get::<Option<String>,_>("last_validate_detail")})
    }).collect();
    Ok(Json(
        json!({"credentials":credentials,"vaultConfigured":state.vault.is_some()}),
    ))
}
fn verdict_json(verdict: &Verdict) -> Value {
    let (code, field, detail) = match verdict {
        Verdict::Ok => (None, None, None),
        Verdict::Limited {
            code,
            field,
            detail,
        }
        | Verdict::Rejected {
            code,
            field,
            detail,
        } => (Some(*code), *field, Some(detail.clone())),
        Verdict::Unreachable { detail } | Verdict::NotImplemented { detail } => {
            (None, None, Some(detail.clone()))
        }
    };
    json!({"status":verdict.status(),"code":code,"field":field,"detail":detail,"safeToActivate":verdict.safe_to_activate()})
}
pub async fn probe(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path((project, id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>, Error> {
    super::tokens::ensure_project_access(&state, &ctx, project).await?;
    let row=sqlx::query("SELECT kind,config,secret_blob,secret_format,version FROM push_credentials WHERE id=$1 AND project_id=$2").bind(id).bind(project).fetch_optional(&state.pool).await.map_err(database_error)?.ok_or_else(||error(StatusCode::NOT_FOUND,"credential_not_found"))?;
    let kind: String = row.get("kind");
    let version: i64 = row.get("version");
    let secret = crate::push_secrets::open(
        state.vault.as_deref(),
        project,
        id,
        &kind,
        &row.get::<String, _>("secret_format"),
        &row.get::<Vec<u8>, _>("secret_blob"),
    )
    .map_err(|code| error(StatusCode::SERVICE_UNAVAILABLE, code))?;
    let verdict = tokio::time::timeout(
        std::time::Duration::from_secs(20),
        crate::push_credential_probe::probe(&kind, &row.get::<Value, _>("config"), &secret),
    )
    .await
    .unwrap_or(Verdict::Unreachable {
        detail: "probe_timeout".into(),
    });
    let response = verdict_json(&verdict);
    // Persist only the stable explanation returned by the probe, never a secret or full URL
    // from an underlying reqwest transport error.
    let detail = response["code"].as_str().or(Some(verdict.status()));
    let mut tx = state.pool.begin().await.map_err(database_error)?;
    push_commands::lock(&mut tx, project).await?;
    super::tokens::ensure_project_access(&state, &ctx, project).await?;
    let changed=sqlx::query("UPDATE push_credentials SET last_validated_at=now(),last_validate_status=$3,last_validate_detail=$4 WHERE id=$1 AND project_id=$2 AND version=$5").bind(id).bind(project).bind(verdict.status()).bind(detail).bind(version).execute(&mut *tx).await.map_err(database_error)?;
    if changed.rows_affected() != 1 {
        return Err(error(StatusCode::CONFLICT, "push_changed"));
    }
    crate::audit::record_in_transaction(
        &mut tx,
        Some(project),
        ctx.user_id,
        "push_credentials.probe",
        "push_credentials",
        &id.to_string(),
        json!({"status":verdict.status()}),
    )
    .await
    .map_err(database_error)?;
    tx.commit().await.map_err(database_error)?;
    // Transport error text can contain credential URLs. A stable code is sufficient here.
    let mut response = response;
    if matches!(verdict, Verdict::Unreachable { .. }) {
        response["detail"] = json!("provider_unreachable");
    }
    Ok(Json(response))
}
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ActivateBody {
    #[serde(default)]
    pub force: bool,
    pub expected_version: Option<i64>,
    pub command_id: Option<String>,
}
pub async fn activate(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path((project, id)): Path<(Uuid, Uuid)>,
    Json(body): Json<ActivateBody>,
) -> Result<Json<Value>, Error> {
    push_commands::execute(
        &state,
        &ctx,
        project,
        Command {
            command_id: body
                .command_id
                .unwrap_or_else(|| Uuid::now_v7().to_string()),
            action: Action::ActivateCredential {
                id,
                expected_version: body.expected_version,
                force: body.force,
            },
        },
    )
    .await
    .map(Json)
}
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeleteQuery {
    pub expected_version: Option<i64>,
    pub command_id: Option<String>,
}
pub async fn delete(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path((project, id)): Path<(Uuid, Uuid)>,
    Query(query): Query<DeleteQuery>,
) -> Result<StatusCode, Error> {
    push_commands::execute(
        &state,
        &ctx,
        project,
        Command {
            command_id: query
                .command_id
                .unwrap_or_else(|| Uuid::now_v7().to_string()),
            action: Action::DeleteCredential {
                id,
                expected_version: query.expected_version,
            },
        },
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
