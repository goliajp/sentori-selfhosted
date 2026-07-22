//! GET    /v1/issues/:issue_id/watchers — list watchers
//! POST   /v1/issues/:issue_id/watchers — current user joins
//! DELETE /v1/issues/:issue_id/watchers — current user leaves
//!
//! In v0.2 the current user is whoever the session middleware
//! resolved. For SDK-side (no session) consumers, watcher writes
//! are 401; reads are 200.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Extension, Path, State},
    http::StatusCode,
};
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

use crate::session_mw::SessionContext;
use crate::state::AppState;

pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(issue_id): Path<Uuid>,
) -> Result<Json<Value>, super::tenant::ApiErr> {
    super::tenant::guard_issue(&state, ctx.workspace_id, issue_id).await?;

    let rows = sqlx::query(
        "SELECT user_id, since AS started_at FROM watchers WHERE issue_id = $1 ORDER BY since",
    )
    .bind(issue_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();
    let out: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "user_id": r.get::<Uuid, _>("user_id").to_string(),
                "started_at": crate::wire_time::rfc3339(r.get::<time::OffsetDateTime, _>("started_at")),
            })
        })
        .collect();
    Ok(Json(json!({ "watchers": out })))
}

pub async fn join(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(issue_id): Path<Uuid>,
) -> Result<StatusCode, super::tenant::ApiErr> {
    super::tenant::guard_issue(&state, ctx.workspace_id, issue_id).await?;

    let _ = sqlx::query(
        "INSERT INTO watchers (issue_id, user_id) VALUES ($1, $2) \
         ON CONFLICT (issue_id, user_id) DO NOTHING",
    )
    .bind(issue_id)
    .bind(ctx.user_id.into_uuid())
    .execute(&state.pool)
    .await;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn leave(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(issue_id): Path<Uuid>,
) -> Result<StatusCode, super::tenant::ApiErr> {
    super::tenant::guard_issue(&state, ctx.workspace_id, issue_id).await?;

    let _ = sqlx::query("DELETE FROM watchers WHERE issue_id = $1 AND user_id = $2")
        .bind(issue_id)
        .bind(ctx.user_id.into_uuid())
        .execute(&state.pool)
        .await;
    Ok(StatusCode::NO_CONTENT)
}
