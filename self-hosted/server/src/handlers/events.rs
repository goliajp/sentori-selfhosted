//! GET /v1/projects/:project_id/events — recent event tail, and
//! GET /v1/projects/:project_id/events/:event_id — one event in full.
//!
//! The single-event read is what a crash view is built on. Ingest
//! stores the SDK's entire JSON verbatim in `events.payload`, so that
//! one column carries the stack frames (with their pre/post source
//! context and the recursive `cause` chain), the breadcrumb timeline,
//! device / app / bundle / user / tags / flags — everything the SDK
//! bothered to collect. None of it had ever been readable: no endpoint
//! selected `payload`, and there was no route for a single event at
//! all, so the dashboard could only ever list rows.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::state::AppState;

#[derive(Deserialize, Default)]
pub struct ListQuery {
    /// Optional issue filter.
    pub issue_id: Option<Uuid>,
    /// Max rows (default 50, max 500).
    pub limit: Option<u32>,
}

#[derive(Serialize)]
pub struct EventRow {
    pub id: Uuid,
    pub issue_id: Uuid,
    pub kind: String,
    #[serde(with = "time::serde::rfc3339")]
    pub timestamp: OffsetDateTime,
    pub release: String,
    pub environment: String,
    pub platform: String,
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<crate::session_mw::SessionContext>,
    Path(project_id): Path<Uuid>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Vec<EventRow>>, (StatusCode, String)> {
    super::tenant::guard_project(&state, ctx.workspace_id, project_id).await?;

    let limit = q.limit.unwrap_or(50).min(500);

    // The two branches carry different parameter counts, so each
    // builds its own bind chain rather than sharing one with a
    // placeholder for the absent issue filter.
    let rows: Vec<(Uuid, Uuid, String, OffsetDateTime, String, String, String)> =
        if let Some(iid) = q.issue_id {
            sqlx::query_as(
                "SELECT id, issue_id, kind, timestamp, release, environment, platform
                 FROM events
                 WHERE project_id = $1 AND workspace_id = $2 AND issue_id = $3
                 ORDER BY timestamp DESC LIMIT $4",
            )
            .bind(project_id)
            .bind(ctx.workspace_id.into_uuid())
            .bind(iid)
            .bind(i64::from(limit))
            .fetch_all(&state.pool)
            .await
        } else {
            sqlx::query_as(
                "SELECT id, issue_id, kind, timestamp, release, environment, platform
                 FROM events
                 WHERE project_id = $1 AND workspace_id = $2
                 ORDER BY timestamp DESC LIMIT $3",
            )
            .bind(project_id)
            .bind(ctx.workspace_id.into_uuid())
            .bind(i64::from(limit))
            .fetch_all(&state.pool)
            .await
        }
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(
        rows.into_iter()
            .map(
                |(id, issue_id, kind, timestamp, release, environment, platform)| EventRow {
                    id,
                    issue_id,
                    kind,
                    timestamp,
                    release,
                    environment,
                    platform,
                },
            )
            .collect(),
    ))
}

/// GET /v1/projects/:project_id/events/trend?days=N
///
/// Returns `[{ day: "YYYY-MM-DD", count: N }]` for the last
/// `days` days (default 7, max 90). Used by the dashboard
/// Overview sparkline.
pub async fn trend(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<crate::session_mw::SessionContext>,
    Path(project_id): Path<Uuid>,
    Query(q): Query<TrendQuery>,
) -> Result<Json<Vec<TrendRow>>, (StatusCode, String)> {
    super::tenant::guard_project(&state, ctx.workspace_id, project_id).await?;

    let days = i64::from(q.days.unwrap_or(7).clamp(1, 90));
    let rows: Vec<(time::Date, i64)> = sqlx::query_as(
        "SELECT (received_at AT TIME ZONE 'UTC')::date AS day, COUNT(*)::bigint \
         FROM events \
         WHERE project_id = $1 AND workspace_id = $2 \
           AND received_at >= now() - ($3 || ' days')::interval \
         GROUP BY day ORDER BY day",
    )
    .bind(project_id)
    .bind(ctx.workspace_id.into_uuid())
    .bind(days.to_string())
    .fetch_all(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(
        rows.into_iter()
            .map(|(day, count)| TrendRow {
                day: day.to_string(),
                count,
            })
            .collect(),
    ))
}

#[derive(Deserialize, Default)]
pub struct TrendQuery {
    pub days: Option<u32>,
}

#[derive(Serialize)]
pub struct TrendRow {
    pub day: String,
    pub count: i64,
}

/// One event, in full: the typed columns, the verbatim SDK payload,
/// and the attachments actually on file for it.
///
/// The attachment list is read from `event_attachments` rather than
/// trusted from `payload.attachments` — the payload echo is whatever
/// the SDK believed it uploaded, while the table is what survived. A
/// `ref` from here always resolves through
/// `GET /v1/projects/{pid}/attachments/{ref}`.
pub async fn get(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<crate::session_mw::SessionContext>,
    Path((project_id, event_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>, (StatusCode, String)> {
    super::tenant::guard_project(&state, ctx.workspace_id, project_id).await?;

    let row = sqlx::query(
        "SELECT id, issue_id, kind, timestamp, release, environment, platform, \
                received_at, payload \
         FROM events \
         WHERE id = $1 AND project_id = $2 AND workspace_id = $3",
    )
    .bind(event_id)
    .bind(project_id)
    .bind(ctx.workspace_id.into_uuid())
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "event not found".to_string()))?;

    let attachments = sqlx::query(
        "SELECT ref, kind, media_type, size_bytes, captured_at, source \
         FROM event_attachments \
         WHERE event_id = $1 AND project_id = $2 AND workspace_id = $3 \
         ORDER BY captured_at DESC",
    )
    .bind(event_id)
    .bind(project_id)
    .bind(ctx.workspace_id.into_uuid())
    .fetch_all(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .iter()
    .map(|a| {
        json!({
            "ref": a.get::<Uuid, _>("ref").to_string(),
            "kind": a.get::<String, _>("kind"),
            "media_type": a.get::<String, _>("media_type"),
            "size_bytes": a.get::<i32, _>("size_bytes"),
            "captured_at": crate::wire_time::rfc3339(a.get::<OffsetDateTime, _>("captured_at")),
            "source": a.get::<String, _>("source"),
        })
    })
    .collect::<Vec<_>>();

    Ok(Json(json!({
        "id": row.get::<Uuid, _>("id").to_string(),
        "issue_id": row.get::<Uuid, _>("issue_id").to_string(),
        "kind": row.get::<String, _>("kind"),
        "timestamp": crate::wire_time::rfc3339(row.get::<OffsetDateTime, _>("timestamp")),
        "received_at": crate::wire_time::rfc3339(row.get::<OffsetDateTime, _>("received_at")),
        "release": row.get::<String, _>("release"),
        "environment": row.get::<String, _>("environment"),
        "platform": row.get::<String, _>("platform"),
        "payload": row.get::<Value, _>("payload"),
        "attachments": attachments,
    })))
}
