//! GET /v1/projects/:project_id/stats — 24h counts for each lens
//! (events / issues / spans / metrics / replays). Used by the
//! Overview page to render per-project lens chips.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

use crate::state::AppState;

pub async fn project_stats(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<crate::session_mw::SessionContext>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Value>, (StatusCode, String)> {
    super::tenant::guard_project(&state, ctx.workspace_id, project_id).await?;

    let workspace_id = ctx.workspace_id.into_uuid();

    // Each count uses partition-pruning filters (`received_at >=
    // now() - interval`) so they stay cheap on partitioned tables.
    let events: i64 = scalar_count(
        &state,
        "SELECT COUNT(*)::bigint FROM events WHERE project_id = $1 AND workspace_id = $2 AND received_at >= now() - interval '24 hours'",
        project_id,
        workspace_id,
    )
    .await?;
    let issues_active: i64 = scalar_count(
        &state,
        "SELECT COUNT(*)::bigint FROM issues WHERE project_id = $1 AND workspace_id = $2 AND status = 'active'",
        project_id,
        workspace_id,
    )
    .await?;
    let spans: i64 = scalar_count(
        &state,
        "SELECT COUNT(*)::bigint FROM spans WHERE project_id = $1 AND workspace_id = $2 AND received_at >= now() - interval '24 hours'",
        project_id,
        workspace_id,
    )
    .await?;
    let metrics_buckets: i64 = scalar_count(
        &state,
        "SELECT COUNT(*)::bigint FROM metrics WHERE project_id = $1 AND workspace_id = $2 AND ts >= now() - interval '24 hours'",
        project_id,
        workspace_id,
    )
    .await?;
    let replays: i64 = scalar_count(
        &state,
        "SELECT COUNT(*)::bigint FROM replay_sessions WHERE project_id = $1 AND workspace_id = $2 AND created_at >= now() - interval '24 hours'",
        project_id,
        workspace_id,
    )
    .await?;
    Ok(Json(json!({
        "events_24h": events,
        "issues_active": issues_active,
        "spans_24h": spans,
        "metrics_buckets_24h": metrics_buckets,
        "replays_24h": replays,
    })))
}

async fn scalar_count(
    state: &Arc<AppState>,
    sql: &str,
    project_id: Uuid,
    workspace_id: Uuid,
) -> Result<i64, (StatusCode, String)> {
    let row = sqlx::query(sql)
        .bind(project_id)
        .bind(workspace_id)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(row.try_get::<i64, _>(0).unwrap_or(0))
}
