//! GET /v1/projects/:project_id/metrics — distinct metric names + last value
//! GET /v1/projects/:project_id/metrics/:name/timeseries — minute rollup

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

pub async fn list_names(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<crate::session_mw::SessionContext>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Value>, (StatusCode, String)> {
    super::tenant::guard_project(&state, ctx.workspace_id, project_id).await?;

    // v0.2 stores per-row metrics; aggregate at read time.
    let rows = sqlx::query(
        "SELECT name, MAX(ts) AS last_bucket, COUNT(*)::bigint AS total_count, \
                AVG(value)::float8 AS avg_value \
         FROM metrics \
         WHERE project_id = $1 AND workspace_id = $2 \
           AND ts >= now() - interval '24 hours' \
         GROUP BY name ORDER BY last_bucket DESC LIMIT 100",
    )
    .bind(project_id)
    .bind(ctx.workspace_id.into_uuid())
    .fetch_all(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let out: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "name": r.get::<String, _>("name"),
                "last_bucket": crate::wire_time::rfc3339_opt(r.try_get::<Option<OffsetDateTime>, _>("last_bucket").ok().flatten()),
                "total_count": r.try_get::<i64, _>("total_count").unwrap_or(0),
                "avg_value": r.try_get::<f64, _>("avg_value").unwrap_or(0.0),
            })
        })
        .collect();
    Ok(Json(json!({ "metrics": out })))
}

#[derive(Deserialize, Default)]
pub struct SeriesQuery {
    pub hours: Option<u32>,
}

pub async fn timeseries(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<crate::session_mw::SessionContext>,
    Path((project_id, name)): Path<(Uuid, String)>,
    Query(q): Query<SeriesQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    super::tenant::guard_project(&state, ctx.workspace_id, project_id).await?;

    let hours = i64::from(q.hours.unwrap_or(24).clamp(1, 720));
    let rows = sqlx::query(
        "SELECT date_trunc('minute', ts) AS bucket, SUM(value)::float8 AS sum, \
                COUNT(*)::bigint AS count, MIN(value)::float8 AS min, \
                MAX(value)::float8 AS max \
         FROM metrics \
         WHERE project_id = $1 AND workspace_id = $2 AND name = $3 \
           AND ts >= now() - ($4 || ' hours')::interval \
         GROUP BY bucket ORDER BY bucket",
    )
    .bind(project_id)
    .bind(ctx.workspace_id.into_uuid())
    .bind(&name)
    .bind(hours.to_string())
    .fetch_all(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let out: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "bucket": crate::wire_time::rfc3339(r.get::<OffsetDateTime, _>("bucket")),
                "sum": r.try_get::<f64, _>("sum").unwrap_or(0.0),
                "count": r.try_get::<i64, _>("count").unwrap_or(0),
                "min": r.try_get::<Option<f64>, _>("min").ok().flatten(),
                "max": r.try_get::<Option<f64>, _>("max").ok().flatten(),
            })
        })
        .collect();
    Ok(Json(json!({ "name": name, "hours": hours, "points": out })))
}
