//! POST `/v1/push/sends/{send_id}/ack` — mark push as user-confirmed.

use std::sync::Arc;

use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};
use sentori_ingest_token::IngestContext;
use serde::Deserialize;
use serde_json::{Value, json};
use tracing::warn;
use uuid::Uuid;

use crate::state::AppState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AckBody {
    /// Caller-supplied session id so multiple opens of the same
    /// notification don't double-record.
    #[serde(default)]
    pub ack_session_id: Option<String>,
}

pub async fn handle(
    Extension(ctx): Extension<IngestContext>,
    State(state): State<Arc<AppState>>,
    Path(send_id): Path<Uuid>,
    Json(body): Json<AckBody>,
) -> (StatusCode, Json<Value>) {
    let result = sqlx::query(
        "UPDATE push_sends SET acked_at = now(), ack_session_id = $1 \
         WHERE id = $2 AND project_id = $3 AND acked_at IS NULL",
    )
    .bind(body.ack_session_id.as_deref())
    .bind(send_id)
    .bind(ctx.project_id.into_uuid())
    .execute(&state.pool)
    .await;

    match result {
        Ok(_) => (StatusCode::ACCEPTED, Json(json!({ "status": "acked" }))),
        Err(e) => {
            warn!(error = %e, "push.ack db_error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}
