//! Reading a crash's attachments back out.
//!
//! - `GET /v1/projects/:project_id/events/:event_id/attachments`
//!   — what evidence exists for this event.
//! - `GET /v1/projects/:project_id/attachments/:ref`
//!   — the bytes, streamed with their captured media type.
//!
//! Until now `event_attachments` had no read surface at all: the SDK
//! uploaded screenshots, view trees, state snapshots, log tails,
//! session trails and replay recordings, and none of it could ever be
//! served back. These two endpoints are what the crash detail view is
//! built on — the replay player fetches the NDJSON through the second
//! one.
//!
//! Both are session-gated and workspace-scoped: the `ref` lookup is
//! constrained by the caller's workspace as well as the project in the
//! path, so an attachment id from another tenant resolves to 404.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Extension, Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use sentori_attachment_store::BlobHash;
use serde_json::{Value, json};
use sqlx::Row;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::session_mw::SessionContext;
use crate::state::AppState;

/// List every attachment captured for one event, newest first.
pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path((project_id, event_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>, (StatusCode, String)> {
    super::tenant::guard_project(&state, ctx.workspace_id, project_id).await?;

    let rows = sqlx::query(
        "SELECT ref, kind, media_type, size_bytes, captured_at, source, received_at \
         FROM event_attachments \
         WHERE event_id = $1 AND project_id = $2 AND workspace_id = $3 \
         ORDER BY captured_at DESC",
    )
    .bind(event_id)
    .bind(project_id)
    .bind(ctx.workspace_id.into_uuid())
    .fetch_all(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let out: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "ref": r.get::<Uuid, _>("ref").to_string(),
                "kind": r.get::<String, _>("kind"),
                "media_type": r.get::<String, _>("media_type"),
                "size_bytes": r.get::<i32, _>("size_bytes"),
                "captured_at": crate::wire_time::rfc3339(r.get::<OffsetDateTime, _>("captured_at")),
                "source": r.get::<String, _>("source"),
                "received_at": crate::wire_time::rfc3339(r.get::<OffsetDateTime, _>("received_at")),
            })
        })
        .collect();
    Ok(Json(json!({ "attachments": out })))
}

/// Stream one attachment's bytes.
pub async fn get(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path((project_id, reference)): Path<(Uuid, Uuid)>,
) -> Result<Response, (StatusCode, String)> {
    super::tenant::guard_project(&state, ctx.workspace_id, project_id).await?;

    // The row carries both the media type and the content hash; the
    // workspace + project predicates are the tenancy check.
    let row = sqlx::query(
        "SELECT blob_hash, media_type FROM event_attachments \
         WHERE ref = $1 AND project_id = $2 AND workspace_id = $3",
    )
    .bind(reference)
    .bind(project_id)
    .bind(ctx.workspace_id.into_uuid())
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "attachment not found".to_string()))?;

    let blob_hash: String = row.get("blob_hash");
    let media_type: String = row.get("media_type");
    let hash = BlobHash::from_hex(&blob_hash)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let bytes = state
        .attachments
        .get(&hash)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))?;

    let mut resp = bytes.into_response();
    if let Ok(v) = HeaderValue::from_str(&media_type) {
        resp.headers_mut().insert(header::CONTENT_TYPE, v);
    }
    Ok(resp)
}
