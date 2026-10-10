//! Versioned, encrypted channel configuration and explicit, durable delivery commands.
use super::jobs::database_error;
use crate::{
    integrations::{self, Settings},
    session_mw::SessionContext,
    state::AppState,
};
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Row, Transaction};
use std::sync::Arc;
use uuid::Uuid;
type Error = (StatusCode, Json<Value>);
fn error(status: StatusCode, code: &str) -> Error {
    (status, Json(json!({"error":code})))
}
fn superadmin_only(ctx: &SessionContext) -> Result<(), Error> {
    if ctx.role.is_superadmin() {
        Ok(())
    } else {
        Err(error(StatusCode::FORBIDDEN, "owner_required"))
    }
}
pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project): Path<Uuid>,
) -> Result<Json<Value>, Error> {
    super::admin::tokens::ensure_project_access(&state, &ctx, project).await?;
    let items:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('id',id,'kind',kind,'enabled',enabled,'events',events,'summary',summary,'version',version,'secretConfigured',true) FROM outbound_integrations WHERE project_id=$1 ORDER BY kind COLLATE \"C\"").bind(project).fetch_all(&state.pool).await.map_err(database_error)?;
    Ok(Json(
        json!({"integrations":items,"vaultConfigured":state.vault.is_some(),"smtpConfigured":state.mailer.transport().is_some()}),
    ))
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Configure {
    command_id: Uuid,
    expected_version: i64,
    enabled: bool,
    events: Vec<String>,
    settings: Settings,
}
async fn lock_project(tx: &mut Transaction<'_, Postgres>, project: Uuid) -> Result<(), Error> {
    sqlx::query("SELECT id FROM projects WHERE id=$1 FOR NO KEY UPDATE")
        .bind(project)
        .fetch_optional(&mut **tx)
        .await
        .map_err(database_error)?
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "not_found"))?;
    Ok(())
}
async fn prior(
    tx: &mut Transaction<'_, Postgres>,
    project: Uuid,
    actor: Uuid,
    key: Uuid,
    digest: &str,
) -> Result<Option<Value>, Error> {
    let row=sqlx::query("SELECT digest,response FROM integration_commands WHERE project_id=$1 AND actor_user_id=$2 AND command_id=$3").bind(project).bind(actor).bind(key).fetch_optional(&mut **tx).await.map_err(database_error)?;
    row.map(|r| {
        if r.get::<String, _>("digest") == digest {
            Ok(r.get("response"))
        } else {
            Err(error(StatusCode::CONFLICT, "idempotency_conflict"))
        }
    })
    .transpose()
}
async fn record(
    tx: &mut Transaction<'_, Postgres>,
    project: Uuid,
    actor: Uuid,
    key: Uuid,
    digest: &str,
    response: &Value,
) -> Result<(), Error> {
    sqlx::query("INSERT INTO integration_commands(project_id,actor_user_id,command_id,digest,response) VALUES($1,$2,$3,$4,$5)").bind(project).bind(actor).bind(key).bind(digest).bind(response).execute(&mut **tx).await.map_err(database_error)?;
    Ok(())
}
fn digest(value: &impl Serialize) -> Result<String, Error> {
    Ok(hex::encode(Sha256::digest(
        serde_json::to_vec(value).map_err(|_| error(StatusCode::BAD_REQUEST, "invalid_command"))?,
    )))
}
pub async fn configure(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path((project, kind)): Path<(Uuid, String)>,
    Json(mut body): Json<Configure>,
) -> Result<Json<Value>, Error> {
    superadmin_only(&ctx)?;
    let vault = state
        .vault
        .as_ref()
        .ok_or_else(|| error(StatusCode::SERVICE_UNAVAILABLE, "vault_unconfigured"))?;
    if !matches!(kind.as_str(), "email" | "webhook" | "github" | "jira")
        || body.events.len() > integrations::EVENTS.len()
        || body
            .events
            .iter()
            .any(|v| !integrations::EVENTS.contains(&v.as_str()))
    {
        return Err(error(StatusCode::BAD_REQUEST, "invalid_integration"));
    }
    let digest = digest(&json!({"kind":kind,"configure":body}))?;
    let mut tx = state.pool.begin().await.map_err(database_error)?;
    lock_project(&mut tx, project).await?;
    if let Some(response) = prior(&mut tx, project, ctx.user_id, body.command_id, &digest).await? {
        return Ok(Json(response));
    }
    let old=sqlx::query("SELECT id,version,secret_blob FROM outbound_integrations WHERE project_id=$1 AND kind=$2 FOR UPDATE").bind(project).bind(&kind).fetch_optional(&mut *tx).await.map_err(database_error)?;
    let version = old.as_ref().map_or(0, |r| r.get::<i64, _>("version"));
    if version != body.expected_version {
        return Err(error(StatusCode::CONFLICT, "integration_changed"));
    }
    if let Some(old) = &old
        && kind != "email"
        && (body.settings.token.is_empty()
            || (matches!(kind.as_str(), "webhook" | "jira") && body.settings.endpoint.is_empty()))
    {
        let settings =
            integrations::open(vault, project, &kind, &old.get::<Vec<u8>, _>("secret_blob"))
                .map_err(|code| error(StatusCode::CONFLICT, code))?;
        body.settings.merge_secrets(&settings);
    }
    body.settings
        .validate(&kind)
        .map_err(|code| error(StatusCode::BAD_REQUEST, code))?;
    let blob = integrations::seal(vault, project, &kind, &body.settings)
        .map_err(|code| error(StatusCode::INTERNAL_SERVER_ERROR, code))?;
    let id = old.as_ref().map_or_else(Uuid::now_v7, |r| r.get("id"));
    sqlx::query("INSERT INTO outbound_integrations(id,project_id,kind,enabled,events,summary,secret_blob) VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT(project_id,kind) DO UPDATE SET enabled=$4,events=$5,summary=$6,secret_blob=$7,version=outbound_integrations.version+1,updated_at=now()")
        .bind(id).bind(project).bind(&kind).bind(body.enabled).bind(body.events).bind(body.settings.summary(&kind))
        .bind(blob).execute(&mut *tx).await.map_err(database_error)?;
    let response = json!({"id":id,"version":version+1});
    record(
        &mut tx,
        project,
        ctx.user_id,
        body.command_id,
        &digest,
        &response,
    )
    .await?;
    crate::audit::record_in_transaction(
        &mut tx,
        Some(project),
        ctx.user_id,
        "integration.configured",
        "integration",
        &id.to_string(),
        json!({"version":version+1}),
    )
    .await
    .map_err(database_error)?;
    tx.commit().await.map_err(database_error)?;
    Ok(Json(response))
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Send {
    command_id: Uuid,
    expected_version: i64,
    task_id: Option<Uuid>,
    expected_task_version: Option<i64>,
}
pub async fn send(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path((project, id)): Path<(Uuid, Uuid)>,
    Json(body): Json<Send>,
) -> Result<Json<Value>, Error> {
    superadmin_only(&ctx)?;
    let digest = digest(&json!({"send":body,"id":id}))?;
    let mut tx = state.pool.begin().await.map_err(database_error)?;
    lock_project(&mut tx, project).await?;
    if let Some(response) = prior(&mut tx, project, ctx.user_id, body.command_id, &digest).await? {
        return Ok(Json(response));
    }
    sqlx::query("SELECT id FROM outbound_integrations WHERE id=$1 AND project_id=$2 AND version=$3 AND enabled FOR UPDATE").bind(id).bind(project).bind(body.expected_version).fetch_optional(&mut *tx).await.map_err(database_error)?.ok_or_else(||error(StatusCode::CONFLICT,"integration_changed"))?;
    let payload = if let Some(task) = body.task_id {
        let row = sqlx::query(
            "SELECT title,description FROM tasks WHERE id=$1 AND project_id=$2 AND version=$3",
        )
        .bind(task)
        .bind(project)
        .bind(body.expected_task_version)
        .fetch_optional(&mut *tx)
        .await
        .map_err(database_error)?
        .ok_or_else(|| error(StatusCode::CONFLICT, "task_changed_or_unavailable"))?;
        json!({"event":"task.export","title":row.get::<String,_>("title"),"body":row.get::<String,_>("description"),"route":format!("/tasks?project={project}&id={task}")})
    } else {
        json!({"event":"integration.test","title":"Sentori integration test","body":"This delivery was requested by the workspace owner.","route":format!("/integrations?project={project}")})
    };
    let event = crate::notification_queue::enqueue(
        &mut tx,
        crate::notification_queue::Event {
            project,
            key: format!("send:{}", body.command_id),
            kind: payload["event"]
                .as_str()
                .unwrap_or("integration.test")
                .into(),
            payload,
            issue: None,
            actor: Some(ctx.user_id),
            target: Some(id),
        },
    )
    .await
    .map_err(database_error)?;
    let delivery = Uuid::now_v7();
    sqlx::query("INSERT INTO notification_deliveries(id,project_id,event_id,integration_id,destination,config_version) VALUES($1,$2,$3,$4,'integration',$5)").bind(delivery).bind(project).bind(event).bind(id).bind(body.expected_version).execute(&mut *tx).await.map_err(database_error)?;
    sqlx::query("UPDATE notification_events SET routed_at=now() WHERE id=$1")
        .bind(event)
        .execute(&mut *tx)
        .await
        .map_err(database_error)?;
    let response = json!({"deliveryId":delivery});
    record(
        &mut tx,
        project,
        ctx.user_id,
        body.command_id,
        &digest,
        &response,
    )
    .await?;
    crate::audit::record_in_transaction(
        &mut tx,
        Some(project),
        ctx.user_id,
        "integration.send_requested",
        "delivery",
        &delivery.to_string(),
        json!({"integrationId":id,"taskId":body.task_id}),
    )
    .await
    .map_err(database_error)?;
    tx.commit().await.map_err(database_error)?;
    Ok(Json(response))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Page {
    offset: Option<i64>,
}
pub async fn deliveries(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project): Path<Uuid>,
    Query(page): Query<Page>,
) -> Result<Json<Value>, Error> {
    super::admin::tokens::ensure_project_access(&state, &ctx, project).await?;
    let offset = page.offset.unwrap_or(0).clamp(0, 1_000_000);
    let rows:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('id',d.id,'integrationId',d.integration_id,'kind',COALESCE(i.kind,CASE WHEN d.destination='user_email' THEN 'email' ELSE 'webhook' END),'event',e.kind,'title',e.payload->>'title','state',d.state,'attempt',d.attempt,'error',d.error_code,'httpStatus',d.http_status,'externalUrl',d.external_url,'createdAt',d.created_at,'finishedAt',d.finished_at) FROM notification_deliveries d JOIN notification_events e ON e.id=d.event_id LEFT JOIN outbound_integrations i ON i.id=d.integration_id WHERE d.project_id=$1 ORDER BY d.created_at DESC,d.id DESC LIMIT 51 OFFSET $2").bind(project).bind(offset).fetch_all(&state.pool).await.map_err(database_error)?;
    Ok(Json(
        json!({"deliveries":rows.iter().take(50).collect::<Vec<_>>(),"nextOffset":(rows.len()>50).then_some(offset+50)}),
    ))
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Retry {
    command_id: Uuid,
    expected_attempt: i32,
    acknowledge_duplicate: bool,
}
pub async fn retry(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path((project, id)): Path<(Uuid, Uuid)>,
    Json(body): Json<Retry>,
) -> Result<Json<Value>, Error> {
    superadmin_only(&ctx)?;
    let digest = digest(&json!({"retry":body,"id":id}))?;
    let mut tx = state.pool.begin().await.map_err(database_error)?;
    lock_project(&mut tx, project).await?;
    if let Some(response) = prior(&mut tx, project, ctx.user_id, body.command_id, &digest).await? {
        return Ok(Json(response));
    }
    let result=sqlx::query("UPDATE notification_deliveries SET state='queued',next_attempt_at=now(),finished_at=NULL,error_code=NULL WHERE id=$1 AND project_id=$2 AND attempt=$3 AND attempt<5 AND (state='failed' OR (state='uncertain' AND $4))")
        .bind(id).bind(project).bind(body.expected_attempt).bind(body.acknowledge_duplicate).execute(&mut *tx).await.map_err(database_error)?;
    if result.rows_affected() != 1 {
        return Err(error(
            StatusCode::CONFLICT,
            "delivery_changed_or_confirmation_required",
        ));
    }
    let response = json!({"deliveryId":id});
    record(
        &mut tx,
        project,
        ctx.user_id,
        body.command_id,
        &digest,
        &response,
    )
    .await?;
    crate::audit::record_in_transaction(
        &mut tx,
        Some(project),
        ctx.user_id,
        "integration.retry_requested",
        "delivery",
        &id.to_string(),
        json!({"acknowledgeDuplicate":body.acknowledge_duplicate}),
    )
    .await
    .map_err(database_error)?;
    tx.commit().await.map_err(database_error)?;
    Ok(Json(response))
}
