//! Reading `track_events` back.
//!
//! The ingest side (`POST /v1/track:batch`) has existed since migration
//! 0019 and production holds 19k rows. Nothing could read them, so the
//! table was write-only: a customer's `$pageview`s and `bio.login.*`
//! events went in and never came out.
//!
//! Three shapes, matching the three indexes the table already carries:
//!
//! - `GET  …/track/names` — which events exist and how often
//!   (`project_id, occurred_at`)
//! - `GET  …/track/series?name=` — one event's volume over time
//!   (`project_id, name, occurred_at`)
//! - `GET  …/track/recent?user=` — the tail, optionally for one user
//!   (`project_id, user_id, occurred_at`)
//!
//! Every query is bounded by a window and a row cap. An analytics table
//! grows without limit, and an endpoint that will happily scan all of
//! it is a way to take the database down from the dashboard.

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

/// Longest window any of these will look back over.
const MAX_DAYS: i64 = 90;
/// Most rows a single response will carry.
const MAX_ROWS: i64 = 500;

#[derive(Deserialize)]
pub struct WindowQuery {
    /// Days to look back. Clamped to 1..=[`MAX_DAYS`].
    #[serde(default)]
    pub days: Option<i64>,
}

#[derive(Deserialize)]
pub struct SeriesQuery {
    pub name: String,
    #[serde(default)]
    pub days: Option<i64>,
}

#[derive(Deserialize)]
pub struct RecentQuery {
    #[serde(default)]
    pub name: Option<String>,
    /// Hashed user handle as stored — this is not an email.
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
}

const fn clamp_days(d: Option<i64>) -> i64 {
    match d {
        Some(n) if n >= 1 && n <= MAX_DAYS => n,
        Some(n) if n > MAX_DAYS => MAX_DAYS,
        _ => 7,
    }
}

/// `GET /v1/projects/:project_id/track/names`
///
/// What this project sends, most frequent first. The first question
/// about an analytics stream is what is in it.
pub async fn names(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
    Query(q): Query<WindowQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    super::tenant::guard_project(&state, ctx.workspace_id, project_id).await?;
    let days = clamp_days(q.days);

    let rows = sqlx::query(
        "SELECT name, count(*) AS total, \
                count(DISTINCT user_id) AS users, \
                max(occurred_at) AS last_seen \
         FROM track_events \
         WHERE project_id = $1 AND occurred_at > now() - ($2 || ' days')::interval \
         GROUP BY name ORDER BY total DESC LIMIT $3",
    )
    .bind(project_id)
    .bind(days.to_string())
    .bind(MAX_ROWS)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| internal(&e))?;

    let out: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "name": r.get::<String, _>("name"),
                "total": r.get::<i64, _>("total"),
                "users": r.get::<i64, _>("users"),
                "last_seen": crate::wire_time::rfc3339_opt(
                    r.try_get::<Option<OffsetDateTime>, _>("last_seen").ok().flatten()
                ),
            })
        })
        .collect();
    Ok(Json(json!({ "days": days, "names": out })))
}

/// `GET /v1/projects/:project_id/track/series?name=…`
///
/// Daily counts for one event. `generate_series` fills the gaps so a
/// day with no events is a zero rather than a missing point — a chart
/// that silently closes gaps lies about what happened.
pub async fn series(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
    Query(q): Query<SeriesQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    super::tenant::guard_project(&state, ctx.workspace_id, project_id).await?;
    let days = clamp_days(q.days);

    let rows = sqlx::query(
        "WITH span AS ( \
             SELECT generate_series( \
                 date_trunc('day', now() - ($2 || ' days')::interval), \
                 date_trunc('day', now()), \
                 '1 day' \
             ) AS day \
         ) \
         SELECT span.day, \
                count(t.id) AS total, \
                count(DISTINCT t.user_id) AS users \
         FROM span \
         LEFT JOIN track_events t \
           ON date_trunc('day', t.occurred_at) = span.day \
          AND t.project_id = $1 AND t.name = $3 \
         GROUP BY span.day ORDER BY span.day",
    )
    .bind(project_id)
    .bind(days.to_string())
    .bind(&q.name)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| internal(&e))?;

    let points: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "day": crate::wire_time::rfc3339(r.get::<OffsetDateTime, _>("day")),
                "total": r.get::<i64, _>("total"),
                "users": r.get::<i64, _>("users"),
            })
        })
        .collect();
    Ok(Json(
        json!({ "name": q.name, "days": days, "points": points }),
    ))
}

/// `GET /v1/projects/:project_id/track/recent`
///
/// The tail, newest first, optionally narrowed to one event name or one
/// user. This is the shape that answers "what was this person doing" —
/// the same question the breadcrumb timeline answers inside a crash,
/// asked across sessions.
pub async fn recent(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
    Query(q): Query<RecentQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    super::tenant::guard_project(&state, ctx.workspace_id, project_id).await?;
    let limit = q.limit.unwrap_or(100).clamp(1, MAX_ROWS);

    let rows = sqlx::query(
        "SELECT id, name, user_id, session_id, route, release, environment, \
                props, occurred_at \
         FROM track_events \
         WHERE project_id = $1 \
           AND ($2::text IS NULL OR name = $2) \
           AND ($3::text IS NULL OR user_id = $3) \
         ORDER BY occurred_at DESC LIMIT $4",
    )
    .bind(project_id)
    .bind(q.name.as_deref())
    .bind(q.user.as_deref())
    .bind(limit)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| internal(&e))?;

    let out: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<Uuid, _>("id"),
                "name": r.get::<String, _>("name"),
                "user_id": r.try_get::<Option<String>, _>("user_id").ok().flatten(),
                "session_id": r.try_get::<Option<Uuid>, _>("session_id").ok().flatten(),
                "route": r.try_get::<Option<String>, _>("route").ok().flatten(),
                "release": r.try_get::<Option<String>, _>("release").ok().flatten(),
                "environment": r.try_get::<Option<String>, _>("environment").ok().flatten(),
                "props": r.try_get::<Option<Value>, _>("props").ok().flatten(),
                "occurred_at": crate::wire_time::rfc3339(
                    r.get::<OffsetDateTime, _>("occurred_at")
                ),
            })
        })
        .collect();
    Ok(Json(json!({ "events": out })))
}

fn internal(e: &sqlx::Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An unbounded window is a way to scan an analytics table from the
    /// dashboard; every entry point has to land inside the cap.
    #[test]
    fn window_is_always_bounded() {
        assert_eq!(clamp_days(None), 7);
        assert_eq!(clamp_days(Some(0)), 7);
        assert_eq!(clamp_days(Some(-5)), 7);
        assert_eq!(clamp_days(Some(1)), 1);
        assert_eq!(clamp_days(Some(30)), 30);
        assert_eq!(clamp_days(Some(MAX_DAYS)), MAX_DAYS);
        assert_eq!(clamp_days(Some(10_000)), MAX_DAYS);
    }
}
