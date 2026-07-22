//! DELETE `/v1/push/tokens/{handle}` — revoke a device token.
//!
//! Resolves `handle` as either:
//! - a UUID (server-side token id, from register_token's response)
//! - or a native_token string (provider's own opaque id)
//!
//! and deletes the matching row from `push_tokens`.

use std::sync::Arc;

use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};
use sentori_ingest_token::IngestContext;
use serde_json::{Value, json};
use tracing::{info, warn};
use uuid::Uuid;

use crate::state::AppState;

pub async fn handle(
    Extension(ctx): Extension<IngestContext>,
    State(state): State<Arc<AppState>>,
    Path(handle): Path<String>,
) -> (StatusCode, Json<Value>) {
    let result = if let Ok(id) = Uuid::parse_str(&handle) {
        state.push_tokens.delete(id).await
    } else {
        // Treat as native_token — direct SQL lookup + delete.
        sqlx::query(
            "DELETE FROM push_tokens \
             WHERE project_id = $1 AND native_token = $2",
        )
        .bind(ctx.project_id.into_uuid())
        .bind(&handle)
        .execute(&state.pool)
        .await
        .map(|_| ())
        .map_err(sentori_push_provider::PushError::Db)
    };

    match result {
        Ok(()) => {
            info!(
                workspace_id = %ctx.workspace_id,
                project_id = %ctx.project_id,
                %handle,
                "push.revoke_token deleted",
            );
            (StatusCode::ACCEPTED, Json(json!({ "status": "revoked" })))
        }
        Err(e) => {
            warn!(workspace_id = %ctx.workspace_id, error = %e, "push.revoke_token db_error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}
