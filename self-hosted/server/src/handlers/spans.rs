//! GET /v1/projects/:project_id/traces — recent traces
//! GET /v1/projects/:project_id/traces/:trace_id — span list

use std::sync::Arc;

use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::state::AppState;

#[derive(Deserialize, Default)]
pub struct ListQuery {
    pub limit: Option<u32>,
}

pub async fn list_traces(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<crate::session_mw::SessionContext>,
    Path(project_id): Path<Uuid>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    super::tenant::guard_project(&state, ctx.workspace_id, project_id).await?;

    let limit = i64::from(q.limit.unwrap_or(50).clamp(1, 500));
    let rows = sqlx::query(
        "SELECT trace_id, root_op, root_name, first_seen, last_seen, span_count, \
                status, duration_ms \
         FROM traces \
         WHERE project_id = $1 AND workspace_id = $2 \
         ORDER BY last_seen DESC LIMIT $3",
    )
    .bind(project_id)
    .bind(ctx.workspace_id.into_uuid())
    .bind(limit)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let out: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "trace_id": r.get::<Uuid, _>("trace_id").to_string(),
                "root_op": r.try_get::<Option<String>, _>("root_op").ok().flatten(),
                "root_name": r.try_get::<Option<String>, _>("root_name").ok().flatten(),
                "first_seen": crate::wire_time::rfc3339(r.get::<OffsetDateTime, _>("first_seen")),
                "last_seen": crate::wire_time::rfc3339(r.get::<OffsetDateTime, _>("last_seen")),
                "span_count": r.try_get::<i32, _>("span_count").unwrap_or(0),
                "status": r.get::<String, _>("status"),
                "duration_ms": r.try_get::<i32, _>("duration_ms").unwrap_or(0),
            })
        })
        .collect();
    Ok(Json(json!({ "traces": out })))
}

pub async fn get_trace(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<crate::session_mw::SessionContext>,
    Path((project_id, trace_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>, (StatusCode, String)> {
    super::tenant::guard_project(&state, ctx.workspace_id, project_id).await?;

    let trace_row = sqlx::query(
        "SELECT trace_id, root_op, root_name, first_seen, last_seen, span_count, \
                status, duration_ms \
         FROM traces WHERE trace_id = $1 AND workspace_id = $2",
    )
    .bind(trace_id)
    .bind(ctx.workspace_id.into_uuid())
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "trace_not_found".to_string()))?;

    let span_rows = sqlx::query(
        "SELECT id, parent_span_id, op, name, status, started_at, duration_ms, tags \
         FROM spans WHERE trace_id = $1 AND workspace_id = $2 \
         ORDER BY started_at LIMIT 500",
    )
    .bind(trace_id)
    .bind(ctx.workspace_id.into_uuid())
    .fetch_all(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let spans: Vec<Value> = span_rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<Uuid, _>("id").to_string(),
                "parent_span_id": r.try_get::<Option<Uuid>, _>("parent_span_id").ok().flatten().map(|u| u.to_string()),
                "op": r.get::<String, _>("op"),
                "name": r.get::<String, _>("name"),
                "status": r.get::<String, _>("status"),
                "started_at": crate::wire_time::rfc3339(r.get::<OffsetDateTime, _>("started_at")),
                "duration_ms": r.try_get::<i32, _>("duration_ms").unwrap_or(0),
                "tags": r.try_get::<Value, _>("tags").unwrap_or(Value::Null),
            })
        })
        .collect();

    Ok(Json(json!({
        "trace": {
            "trace_id": trace_row.get::<Uuid, _>("trace_id").to_string(),
            "root_op": trace_row.try_get::<Option<String>, _>("root_op").ok().flatten(),
            "root_name": trace_row.try_get::<Option<String>, _>("root_name").ok().flatten(),
            "first_seen": crate::wire_time::rfc3339(trace_row.get::<OffsetDateTime, _>("first_seen")),
            "last_seen": crate::wire_time::rfc3339(trace_row.get::<OffsetDateTime, _>("last_seen")),
            "span_count": trace_row.try_get::<i32, _>("span_count").unwrap_or(0),
            "status": trace_row.get::<String, _>("status"),
            "duration_ms": trace_row.try_get::<i32, _>("duration_ms").unwrap_or(0),
        },
        "spans": spans,
    })))
}
