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
use sentori_push_provider::PushError;
use serde_json::{Value, json};
use tracing::{info, warn};
use uuid::Uuid;

use crate::state::AppState;

pub async fn handle(
    Extension(ctx): Extension<IngestContext>,
    State(state): State<Arc<AppState>>,
    Path(handle): Path<String>,
) -> (StatusCode, Json<Value>) {
    // `device_tokens`, and a revocation rather than a delete.
    //
    // This used to delete from `push_tokens` — a table nothing else
    // in the send path reads. Targeting, quarantine and the worker
    // all use `device_tokens`, so the request answered 202 "revoked"
    // and the device stayed perfectly deliverable. insight found it
    // the only way anyone could: they called `unregister`, got
    // `ok=true`, sent to the same handle, and watched it go out.
    //
    // Setting `revoked_at` rather than deleting the row, because that
    // is the column `resolve_targets` filters on and the one
    // `push_quarantine` writes when a provider reports the token
    // dead. Two ways to retire a device that leave different traces
    // is one more than this needs.
    let affected = if let Ok(id) = Uuid::parse_str(&handle) {
        sqlx::query(
            "UPDATE device_tokens SET revoked_at = now() \
             WHERE project_id = $1 AND id = $2 AND revoked_at IS NULL",
        )
        .bind(ctx.project_id)
        .bind(id)
        .execute(&state.pool)
        .await
    } else {
        sqlx::query(
            "UPDATE device_tokens SET revoked_at = now() \
             WHERE project_id = $1 AND native_token = $2 AND revoked_at IS NULL",
        )
        .bind(ctx.project_id)
        .bind(&handle)
        .execute(&state.pool)
        .await
    };
    let result = affected.map(|r| r.rows_affected()).map_err(PushError::Db);

    match result {
        Ok(rows) => {
            info!(
                project_id = %ctx.project_id,
                %handle,
                rows,
                "push.revoke_token revoked",
            );
            // Say which it was. A handle that matched nothing — a
            // typo, another project's, one already revoked — used to
            // answer exactly like a successful revocation, and a
            // caller cannot tell "done" from "did nothing" when both
            // are 202 `{"status":"revoked"}`.
            (
                StatusCode::ACCEPTED,
                Json(json!({
                    "status": if rows > 0 { "revoked" } else { "not_found" },
                })),
            )
        }
        Err(e) => {
            warn!(error = %e, "push.revoke_token db_error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}
