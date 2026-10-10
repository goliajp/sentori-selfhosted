//! Owner-only read-only operational diagnostics. Not a release certification.
use std::sync::Arc;

use axum::{Extension, Json, extract::State, http::StatusCode};
use serde_json::{Value, json};

use crate::{blob_store::AttachmentStore, session_mw::SessionContext, state::AppState};

pub async fn get(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
) -> Result<Json<sentori_snapshot::healthcheck::Report>, (StatusCode, Json<Value>)> {
    let code = if state.cloud.is_some() {
        Some("managed_by_cloud")
    } else if !ctx.role.is_superadmin() {
        Some("owner_required")
    } else {
        None
    };
    if let Some(code) = code {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error":code,"code":code})),
        ));
    }
    let path = match &state.attachments {
        AttachmentStore::Fs(store) => Some(store.root()),
        AttachmentStore::Memory(_) => None,
    };
    Ok(Json(
        sentori_snapshot::healthcheck::inspect(Some(&state.pool), path).await,
    ))
}
