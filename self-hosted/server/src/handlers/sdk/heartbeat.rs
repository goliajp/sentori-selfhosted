//! POST `/v1/heartbeat` — SDK liveness ping.
//!
//! Lightweight endpoint: SDK pings periodically so the server
//! can track activity (rate-limit hot vs cold paths). Returns
//! 204 No Content. No DB write — observability metric only.

use axum::{Extension, http::StatusCode};
use sentori_ingest_token::IngestContext;
use tracing::info;

pub async fn handle(Extension(ctx): Extension<IngestContext>) -> StatusCode {
    info!(
        workspace_id = %ctx.workspace_id,
        project_id = %ctx.project_id,
        "sdk.heartbeat",
    );
    StatusCode::NO_CONTENT
}
