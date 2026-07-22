//! GET /v1/projects/:project_id/replays — recent replay sessions

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

pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<crate::session_mw::SessionContext>,
    Path(project_id): Path<Uuid>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    super::tenant::guard_project(&state, ctx.workspace_id, project_id).await?;

    let limit = i64::from(q.limit.unwrap_or(50).clamp(1, 500));
    let rows = sqlx::query(
        "SELECT id, event_id, blob_hash, started_at, ended_at, frame_count, created_at \
         FROM replay_sessions \
         WHERE project_id = $1 AND workspace_id = $2 \
         ORDER BY created_at DESC LIMIT $3",
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
            let started: OffsetDateTime = r.get("started_at");
            let ended: OffsetDateTime = r.get("ended_at");
            json!({
                "id": r.get::<Uuid, _>("id").to_string(),
                "event_id": r.get::<Uuid, _>("event_id").to_string(),
                "blob_hash": r.get::<String, _>("blob_hash"),
                "started_at": started,
                "ended_at": ended,
                "duration_ms": i64::try_from((ended - started).whole_milliseconds())
                    .unwrap_or(i64::MAX),
                "frame_count": r.try_get::<i32, _>("frame_count").unwrap_or(0),
                "created_at": crate::wire_time::rfc3339(r.get::<OffsetDateTime, _>("created_at")),
            })
        })
        .collect();
    Ok(Json(json!({ "replays": out })))
}

/// GET /v1/projects/:project_id/replays/:replay_id/ndjson
///
/// Streams the decompressed NDJSON frame blob (text/plain).
/// The webapp parses it client-side to render a per-frame
/// timeline. Cap is ~10 MB raw per session (SDK-side limit).
pub async fn ndjson(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<crate::session_mw::SessionContext>,
    Path((project_id, replay_id)): Path<(Uuid, Uuid)>,
) -> Result<axum::response::Response, (StatusCode, String)> {
    use axum::response::IntoResponse;
    super::tenant::guard_project(&state, ctx.workspace_id, project_id).await?;

    // The blob store is addressed by replay id alone, so the session
    // row is what ties this id to the caller's workspace.
    let owned: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM replay_sessions \
         WHERE id = $1 AND project_id = $2 AND workspace_id = $3",
    )
    .bind(replay_id)
    .bind(project_id)
    .bind(ctx.workspace_id.into_uuid())
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if owned.is_none() {
        return Err((StatusCode::NOT_FOUND, "replay not found".to_string()));
    }

    match state.replays.fetch(replay_id).await {
        Ok(bytes) => {
            let mut resp = bytes.into_response();
            resp.headers_mut().insert(
                axum::http::header::CONTENT_TYPE,
                axum::http::HeaderValue::from_static("application/x-ndjson"),
            );
            Ok(resp)
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("not found") {
                Err((StatusCode::NOT_FOUND, msg))
            } else {
                Err((StatusCode::INTERNAL_SERVER_ERROR, msg))
            }
        }
    }
}
