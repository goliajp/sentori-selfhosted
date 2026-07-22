//! POST `/v1/push/tokens/{handle}/topics` — subscribe device to topic.

use std::sync::Arc;

use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};
use sentori_ingest_token::IngestContext;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use tracing::{info, warn};
use uuid::Uuid;

use crate::state::AppState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicBody {
    pub topic: String,
}

pub async fn handle(
    Extension(ctx): Extension<IngestContext>,
    State(state): State<Arc<AppState>>,
    Path(handle): Path<String>,
    Json(body): Json<TopicBody>,
) -> (StatusCode, Json<Value>) {
    if body.topic.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "topic required" })),
        );
    }
    let device_token_id = match resolve_device_token(&state, &handle, &ctx).await {
        Ok(Some(id)) => id,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "device_token_not_found" })),
            );
        }
        Err(e) => {
            warn!(error = %e, "push.subscribe_topic resolve_failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            );
        }
    };

    let result = sqlx::query(
        "INSERT INTO device_topics (device_token_id, topic) \
         VALUES ($1, $2) \
         ON CONFLICT (device_token_id, topic) DO NOTHING",
    )
    .bind(device_token_id)
    .bind(&body.topic)
    .execute(&state.pool)
    .await;

    match result {
        Ok(_) => {
            info!(
                workspace_id = %ctx.workspace_id,
                project_id = %ctx.project_id,
                topic = %body.topic,
                "push.subscribe_topic subscribed",
            );
            (
                StatusCode::ACCEPTED,
                Json(json!({ "status": "subscribed", "topic": body.topic })),
            )
        }
        Err(e) => {
            warn!(error = %e, "push.subscribe_topic db_error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}

pub(crate) async fn resolve_device_token(
    state: &Arc<AppState>,
    handle: &str,
    ctx: &IngestContext,
) -> Result<Option<Uuid>, sqlx::Error> {
    if let Ok(id) = Uuid::parse_str(handle) {
        let row = sqlx::query("SELECT id FROM device_tokens WHERE id = $1 AND project_id = $2")
            .bind(id)
            .bind(ctx.project_id.into_uuid())
            .fetch_optional(&state.pool)
            .await?;
        Ok(row.map(|r| r.get::<Uuid, _>("id")))
    } else {
        let row = sqlx::query(
            "SELECT id FROM device_tokens \
             WHERE project_id = $1 AND native_token = $2",
        )
        .bind(ctx.project_id.into_uuid())
        .bind(handle)
        .fetch_optional(&state.pool)
        .await?;
        Ok(row.map(|r| r.get::<Uuid, _>("id")))
    }
}
