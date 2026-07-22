//! GET `/v1/control/poll` — SDK polls server-side flags.
//!
//! Returns a flat object the SDK can react to without rebuilding.
//! Currently:
//! - `live`: false. Phase C step 7+ wires live-mode control
//!   (per-project flag in DB or env-var driven).
//! - `sample_rate`: 1.0 default. Project-level override comes from
//!   the projects table in a later phase.

use axum::{Extension, Json};
use sentori_ingest_token::IngestContext;
use serde_json::{Value, json};
use tracing::info;

pub async fn handle(Extension(ctx): Extension<IngestContext>) -> Json<Value> {
    info!(
        workspace_id = %ctx.workspace_id,
        project_id = %ctx.project_id,
        "sdk.control_poll",
    );
    Json(json!({
        "live": false,
        "sample_rate": 1.0,
    }))
}
