//! Reading what a user said happened.
//!
//! `POST /v1/user-reports` has always accepted them; nothing read them
//! back, so a person's description of their own crash went into the
//! database and stopped there.
//!
//! This is the one signal in the product that is not machine-generated.
//! A stack trace says where; a user report says what they were trying
//! to do — which is why it is joined to the issue rather than filed
//! somewhere separate.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Extension, Path, Query, State},
    http::StatusCode,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::session_mw::SessionContext;
use crate::state::AppState;

const MAX_ROWS: i64 = 200;

#[derive(Deserialize)]
pub struct ListQuery {
    /// Narrow to one issue — the crash page asks this way.
    #[serde(default)]
    pub issue_id: Option<Uuid>,
    #[serde(default)]
    pub limit: Option<i64>,
}

/// `GET /v1/projects/:project_id/user-reports`
pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    super::tenant::guard_project(&state, ctx.workspace_id, project_id).await?;
    let limit = q.limit.unwrap_or(50).clamp(1, MAX_ROWS);

    let rows = sqlx::query(
        "SELECT id, event_id, issue_id, title, body, email, name, received_at \
         FROM user_reports \
         WHERE project_id = $1 \
           AND ($2::uuid IS NULL OR issue_id = $2) \
         ORDER BY received_at DESC LIMIT $3",
    )
    .bind(project_id)
    .bind(q.issue_id)
    .bind(limit)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let out: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<Uuid, _>("id"),
                "event_id": r.try_get::<Option<Uuid>, _>("event_id").ok().flatten(),
                "issue_id": r.try_get::<Option<Uuid>, _>("issue_id").ok().flatten(),
                "title": r.try_get::<Option<String>, _>("title").ok().flatten(),
                "body": r.try_get::<Option<String>, _>("body").ok().flatten(),
                // The reporter's own contact details, as they typed
                // them. Shown to the operator who has to reply.
                "email": r.try_get::<Option<String>, _>("email").ok().flatten(),
                "name": r.try_get::<Option<String>, _>("name").ok().flatten(),
                "received_at": crate::wire_time::rfc3339(
                    r.get::<OffsetDateTime, _>("received_at")
                ),
            })
        })
        .collect();
    Ok(Json(json!({ "reports": out })))
}
