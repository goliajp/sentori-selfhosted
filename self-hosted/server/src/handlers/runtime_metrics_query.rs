//! Reading the SDK's own performance rollups.
//!
//! `POST /v1/runtime-metrics:batch` has always written into
//! `runtime_metrics_raw`, and a worker rolls that up into `_1h` and
//! `_1d`. Nothing read any of it.
//!
//! This is the data behind the project's first rule — that Sentori must
//! not make the host app stutter. `runtime.cold_start_ms`,
//! `runtime.fps.p50/p95`, `runtime.heap.used_bytes` and
//! `runtime.route_nav_ms` are how an integrator checks that for
//! themselves rather than taking our word for it. A promise nobody can
//! measure is a promise on trust.
//!
//! Reads the rollups, never `_raw`: the raw table is partitioned by day
//! and unbounded, and the rollups are what it exists to produce.

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

/// Longest window, in hours, the hourly rollup will serve.
const MAX_HOURS: i64 = 24 * 30;
const MAX_ROWS: i64 = 1000;

#[derive(Deserialize)]
pub struct NamesQuery {
    #[serde(default)]
    pub hours: Option<i64>,
}

#[derive(Deserialize)]
pub struct SeriesQuery {
    pub name: String,
    #[serde(default)]
    pub hours: Option<i64>,
    /// Narrow to one release — the question is usually "did the last
    /// ship make it worse", which needs one line per release.
    #[serde(default)]
    pub release: Option<String>,
}

const fn clamp_hours(h: Option<i64>) -> i64 {
    match h {
        Some(n) if n >= 1 && n <= MAX_HOURS => n,
        Some(n) if n > MAX_HOURS => MAX_HOURS,
        _ => 24,
    }
}

/// `GET /v1/projects/:project_id/runtime-metrics`
///
/// Which measurements this project reports, with the latest value of
/// each. The list is the answer to "is the SDK reporting at all".
pub async fn names(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
    Query(q): Query<NamesQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    super::tenant::guard_project(&state, ctx.workspace_id, project_id).await?;
    let hours = clamp_hours(q.hours);

    let rows = sqlx::query(
        "SELECT DISTINCT ON (name) \
                name, bucket_ts, release, environment, count, avg, p50, p95, p99 \
         FROM runtime_metrics_1h \
         WHERE project_id = $1 AND bucket_ts > now() - ($2 || ' hours')::interval \
         ORDER BY name, bucket_ts DESC \
         LIMIT $3",
    )
    .bind(project_id)
    .bind(hours.to_string())
    .bind(MAX_ROWS)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| internal(&e))?;

    let out: Vec<Value> = rows.iter().map(row_to_point).collect();
    Ok(Json(json!({ "hours": hours, "metrics": out })))
}

/// `GET /v1/projects/:project_id/runtime-metrics/series?name=…`
///
/// One measurement over time, hourly. Percentiles come back alongside
/// the mean because a mean frame time hides exactly the stutter this
/// data exists to catch.
pub async fn series(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
    Query(q): Query<SeriesQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    super::tenant::guard_project(&state, ctx.workspace_id, project_id).await?;
    let hours = clamp_hours(q.hours);

    let rows = sqlx::query(
        "SELECT bucket_ts, release, environment, count, avg, p50, p95, p99 \
         FROM runtime_metrics_1h \
         WHERE project_id = $1 AND name = $2 \
           AND bucket_ts > now() - ($3 || ' hours')::interval \
           AND ($4::text IS NULL OR release = $4) \
         ORDER BY bucket_ts \
         LIMIT $5",
    )
    .bind(project_id)
    .bind(&q.name)
    .bind(hours.to_string())
    .bind(q.release.as_deref())
    .bind(MAX_ROWS)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| internal(&e))?;

    let points: Vec<Value> = rows.iter().map(row_to_point).collect();
    Ok(Json(json!({
        "name": q.name,
        "hours": hours,
        "release": q.release,
        "points": points,
    })))
}

fn row_to_point(r: &sqlx::postgres::PgRow) -> Value {
    json!({
        "name": r.try_get::<String, _>("name").ok(),
        "bucket_ts": crate::wire_time::rfc3339(r.get::<OffsetDateTime, _>("bucket_ts")),
        "release": r.try_get::<Option<String>, _>("release").ok().flatten(),
        "environment": r.try_get::<Option<String>, _>("environment").ok().flatten(),
        "count": r.get::<i64, _>("count"),
        "avg": r.try_get::<Option<f64>, _>("avg").ok().flatten(),
        "p50": r.try_get::<Option<f64>, _>("p50").ok().flatten(),
        "p95": r.try_get::<Option<f64>, _>("p95").ok().flatten(),
        "p99": r.try_get::<Option<f64>, _>("p99").ok().flatten(),
    })
}

fn internal(e: &sqlx::Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_is_always_bounded() {
        assert_eq!(clamp_hours(None), 24);
        assert_eq!(clamp_hours(Some(0)), 24);
        assert_eq!(clamp_hours(Some(1)), 1);
        assert_eq!(clamp_hours(Some(MAX_HOURS)), MAX_HOURS);
        assert_eq!(clamp_hours(Some(MAX_HOURS + 1)), MAX_HOURS);
    }
}
