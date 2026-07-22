//! GET /v1/projects/:project_id/search?q=foo
//!
//! Cheap LIKE-based search across issues + events for dashboard
//! ⌘K palette. Full-text via pg_trgm GIN index lands in v0.3.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

use crate::state::AppState;

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

const fn default_limit() -> u32 {
    20
}

pub async fn search(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<crate::session_mw::SessionContext>,
    Path(project_id): Path<Uuid>,
    Query(q): Query<SearchQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    super::tenant::guard_project(&state, ctx.workspace_id, project_id).await?;

    let needle = q.q.trim();
    if needle.is_empty() {
        return Ok(Json(json!({ "issues": [], "events": [] })));
    }
    let limit = i64::from(q.limit.clamp(1, 100));
    let pattern = format!("%{needle}%");

    let issues = sqlx::query(
        "SELECT id, error_type, message_sample, status, last_seen \
         FROM issues \
         WHERE project_id = $1 AND workspace_id = $2 \
           AND (error_type ILIKE $3 OR message_sample ILIKE $3 OR fingerprint ILIKE $3) \
         ORDER BY last_seen DESC LIMIT $4",
    )
    .bind(project_id)
    .bind(ctx.workspace_id.into_uuid())
    .bind(&pattern)
    .bind(limit)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let events = sqlx::query(
        "SELECT id, issue_id, kind, release, environment, timestamp \
         FROM events \
         WHERE project_id = $1 AND workspace_id = $2 \
           AND received_at >= now() - interval '7 days' \
           AND (release ILIKE $3 OR environment ILIKE $3) \
         ORDER BY timestamp DESC LIMIT $4",
    )
    .bind(project_id)
    .bind(ctx.workspace_id.into_uuid())
    .bind(&pattern)
    .bind(limit)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let issues_out: Vec<Value> = issues
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<Uuid, _>("id").to_string(),
                "error_type": r.get::<String, _>("error_type"),
                "message_sample": r.try_get::<String, _>("message_sample").unwrap_or_default(),
                "status": r.get::<String, _>("status"),
                "last_seen": crate::wire_time::rfc3339(r.get::<time::OffsetDateTime, _>("last_seen")),
            })
        })
        .collect();
    let events_out: Vec<Value> = events
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<Uuid, _>("id").to_string(),
                "issue_id": r.get::<Uuid, _>("issue_id").to_string(),
                "kind": r.get::<String, _>("kind"),
                "release": r.get::<String, _>("release"),
                "environment": r.get::<String, _>("environment"),
                "timestamp": crate::wire_time::rfc3339(r.get::<time::OffsetDateTime, _>("timestamp")),
            })
        })
        .collect();

    Ok(Json(json!({
        "q": needle,
        "issues": issues_out,
        "events": events_out,
    })))
}
