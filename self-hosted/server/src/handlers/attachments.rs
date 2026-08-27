//! GET /admin/api/attachments/{ref} — attachment bytes (replay
//! playback, screenshots). Session-gated; access follows the
//! owning project.

use std::sync::Arc;

use axum::{
    Extension,
    extract::{Path, State},
    http::{StatusCode, header},
    response::IntoResponse,
};
use sqlx::Row;
use uuid::Uuid;

use crate::session_mw::SessionContext;
use crate::state::AppState;

pub async fn get(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(att_ref): Path<Uuid>,
) -> axum::response::Response {
    let row = sqlx::query(
        "SELECT project_id, media_type, blob_hash FROM event_attachments WHERE ref = $1",
    )
    .bind(att_ref)
    .fetch_optional(&state.pool)
    .await
    .unwrap_or(None);
    let Some(r) = row else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let project_id: Uuid = r.get("project_id");
    if let Err(e) = super::admin::tokens::ensure_project_access(&state, &ctx, project_id).await {
        return e.into_response();
    }
    let media_type: String = r.get("media_type");
    let blob_hash: String = r.get("blob_hash");
    let Ok(hash) = blob_hash.parse() else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    match state.attachments.get(&hash).await {
        Ok(bytes) => (StatusCode::OK, [(header::CONTENT_TYPE, media_type)], bytes).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}
