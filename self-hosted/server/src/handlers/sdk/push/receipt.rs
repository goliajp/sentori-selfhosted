//! GET `/v1/push/deliveries/{deliveryId}` — one device's row.
//!
//! Was `/v1/push/receipts/{send_id}`, which named a per-device row
//! with the same word as the call that produced it. Two ids at two
//! levels sharing one word meant passing the wrong one got a 404 and
//! no hint which.

use std::sync::Arc;

use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};
use sentori_ingest_token::IngestContext;
use serde_json::{Value, json};
use sqlx::Row;
use tracing::warn;
use uuid::Uuid;

use crate::state::AppState;

pub async fn handle(
    Extension(ctx): Extension<IngestContext>,
    State(state): State<Arc<AppState>>,
    Path(delivery_id): Path<Uuid>,
) -> (StatusCode, Json<Value>) {
    let row = sqlx::query(
        "SELECT id, batch_id, token_id, status, provider, provider_outcome, error, \
                sent_at, acked_at, retry_count \
         FROM push_sends WHERE id = $1 AND project_id = $2",
    )
    .bind(delivery_id)
    .bind(ctx.project_id)
    .fetch_optional(&state.pool)
    .await;

    match row {
        Ok(Some(r)) => (
            StatusCode::OK,
            // The shape `/sends/{sendId}/deliveries` returns, because
            // it is the same row. Two routes onto one row used to
            // answer with two different sets of field names.
            Json(json!({
                "deliveryId": delivery_id.to_string(),
                "sendId": r.get::<Option<Uuid>, _>("batch_id").map(|b| b.to_string()),
                "spToken": r.get::<Uuid, _>("token_id").to_string(),
                "status": r.get::<String, _>("status"),
                "provider": r.get::<String, _>("provider"),
                "providerOutcome": r.get::<Option<String>, _>("provider_outcome"),
                "error": r.get::<Option<String>, _>("error"),
                "sentAt": crate::wire_time::rfc3339_opt(r.get::<Option<time::OffsetDateTime>, _>("sent_at")),
                "deliveredAt": crate::wire_time::rfc3339_opt(r.get::<Option<time::OffsetDateTime>, _>("acked_at")),
                "retryCount": r.get::<i32, _>("retry_count"),
            })),
        ),
        // A 404 and a 500, not two more values in `status`. An id that
        // does not exist and a database that is down both used to come
        // back 200 carrying a word, so a typo and an outage read the
        // same to anything checking the field.
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "delivery_not_found" })),
        ),
        Err(e) => {
            warn!(error = %e, "push.receipt db_error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}
