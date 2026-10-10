//! Session-only command APIs; permissions are rechecked on every request.
use crate::{jobs::EnqueueError, session_mw::SessionContext, state::AppState};
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;
type Error = (StatusCode, Json<Value>);

#[allow(
    clippy::needless_pass_by_value,
    reason = "map_err consumes its error; this adapter logs it and returns a sanitized HTTP error"
)]
pub(super) fn database_error(error: sqlx::Error) -> Error {
    tracing::warn!(%error, "job command failed");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error":"internal"})),
    )
}
pub(super) fn enqueue_error(error: EnqueueError) -> Error {
    match error {
        EnqueueError::Database(e) => database_error(e),
        EnqueueError::Conflict => (
            StatusCode::CONFLICT,
            Json(json!({"error":"idempotency_conflict"})),
        ),
        EnqueueError::Full => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error":"job_queue_full"})),
        ),
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Create {
    idempotency_key: String,
}
pub async fn create(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path((project, release)): Path<(Uuid, Uuid)>,
    Json(body): Json<Create>,
) -> Result<(StatusCode, Json<Value>), Error> {
    super::admin::releases::ensure_release_access(&state, &ctx, project, release).await?;
    if body.idempotency_key.is_empty()
        || body.idempotency_key.len() > 128
        || !body.idempotency_key.bytes().all(|b| b.is_ascii_graphic())
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"invalid_idempotency_key"})),
        ));
    }
    let mut tx = state.pool.begin().await.map_err(database_error)?;
    let id = crate::jobs::enqueue(
        &mut tx,
        project,
        release,
        Some(ctx.user_id),
        &body.idempotency_key,
    )
    .await
    .map_err(enqueue_error)?;
    tx.commit().await.map_err(database_error)?;
    Ok((StatusCode::ACCEPTED, Json(json!({"jobId":id}))))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct List {
    release_id: Option<Uuid>,
    limit: Option<i64>,
}
fn wire(row: &sqlx::postgres::PgRow) -> Value {
    json!({"id":row.get::<Uuid,_>("id"),"projectId":row.get::<Uuid,_>("project_id"),"releaseId":row.get::<Uuid,_>("release_id"),"kind":row.get::<String,_>("kind"),"state":row.get::<String,_>("state"),"attempt":row.get::<i32,_>("attempt"),"maxAttempts":row.get::<i32,_>("max_attempts"),"cancelRequested":row.get::<bool,_>("cancel_requested"),"scanned":row.get::<i32,_>("scanned"),"rewritten":row.get::<i32,_>("rewritten"),"frames":row.get::<i32,_>("frames"),"scanLimit":20000,"errorCode":row.get::<Option<String>,_>("error_code"),"createdAt":crate::wire_time::rfc3339(row.get("created_at")),"updatedAt":crate::wire_time::rfc3339(row.get("updated_at")),"finishedAt":row.get::<Option<time::OffsetDateTime>,_>("finished_at").map(crate::wire_time::rfc3339)})
}
pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project): Path<Uuid>,
    Query(query): Query<List>,
) -> Result<Json<Value>, Error> {
    super::admin::tokens::ensure_project_access(&state, &ctx, project).await?;
    let rows=sqlx::query("SELECT * FROM jobs WHERE project_id=$1 AND ($2::uuid IS NULL OR release_id=$2) ORDER BY created_at DESC,id DESC LIMIT $3")
        .bind(project).bind(query.release_id).bind(query.limit.unwrap_or(30).clamp(1,100)).fetch_all(&state.pool).await.map_err(database_error)?;
    Ok(Json(
        json!({"jobs":rows.iter().map(wire).collect::<Vec<_>>()}),
    ))
}
pub async fn get(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path((project, id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>, Error> {
    super::admin::tokens::ensure_project_access(&state, &ctx, project).await?;
    let row = sqlx::query("SELECT * FROM jobs WHERE project_id=$1 AND id=$2")
        .bind(project)
        .bind(id)
        .fetch_optional(&state.pool)
        .await
        .map_err(database_error)?
        .ok_or((StatusCode::NOT_FOUND, Json(json!({"error":"not_found"}))))?;
    Ok(Json(wire(&row)))
}
pub async fn cancel(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path((project, id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>, Error> {
    super::admin::tokens::ensure_project_access(&state, &ctx, project).await?;
    let mut tx = state.pool.begin().await.map_err(database_error)?;
    let row = sqlx::query("SELECT * FROM jobs WHERE project_id=$1 AND id=$2 FOR UPDATE")
        .bind(project)
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(database_error)?
        .ok_or((StatusCode::NOT_FOUND, Json(json!({"error":"not_found"}))))?;
    let status: String = row.get("state");
    if matches!(status.as_str(), "queued" | "retry_wait" | "running")
        && !row.get::<bool, _>("cancel_requested")
    {
        sqlx::query("UPDATE jobs SET cancel_requested=true,state=CASE WHEN state='running' THEN state ELSE 'cancelled' END,finished_at=CASE WHEN state='running' THEN NULL ELSE now() END,updated_at=now() WHERE id=$1").bind(id).execute(&mut *tx).await.map_err(database_error)?;
        crate::audit::record_in_transaction(
            &mut tx,
            Some(project),
            ctx.user_id,
            "job.cancel_requested",
            "job",
            &id.to_string(),
            json!({}),
        )
        .await
        .map_err(database_error)?;
    }
    let row = sqlx::query("SELECT * FROM jobs WHERE id=$1")
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .map_err(database_error)?;
    tx.commit().await.map_err(database_error)?;
    Ok(Json(wire(&row)))
}
