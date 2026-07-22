//! DELETE `/v1/push/tokens/{handle}/topics/{topic}`

use std::sync::Arc;

use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};
use sentori_ingest_token::IngestContext;
use serde_json::{Value, json};
use tracing::{info, warn};

use crate::handlers::sdk::push::subscribe_topic::resolve_device_token;
use crate::state::AppState;

pub async fn handle(
    Extension(ctx): Extension<IngestContext>,
    State(state): State<Arc<AppState>>,
    Path((handle, topic)): Path<(String, String)>,
) -> (StatusCode, Json<Value>) {
    let device_token_id = match resolve_device_token(&state, &handle, &ctx).await {
        Ok(Some(id)) => id,
        Ok(None) => {
            return (
                StatusCode::ACCEPTED,
                Json(json!({ "status": "unsubscribed" })),
            );
        }
        Err(e) => {
            warn!(error = %e, "push.unsubscribe_topic resolve_failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            );
        }
    };

    let result = sqlx::query("DELETE FROM device_topics WHERE device_token_id = $1 AND topic = $2")
        .bind(device_token_id)
        .bind(&topic)
        .execute(&state.pool)
        .await;

    match result {
        Ok(_) => {
            info!(
                workspace_id = %ctx.workspace_id,
                project_id = %ctx.project_id,
                %topic,
                "push.unsubscribe_topic unsubscribed",
            );
            (
                StatusCode::ACCEPTED,
                Json(json!({ "status": "unsubscribed", "topic": topic })),
            )
        }
        Err(e) => {
            warn!(error = %e, "push.unsubscribe_topic db_error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}
