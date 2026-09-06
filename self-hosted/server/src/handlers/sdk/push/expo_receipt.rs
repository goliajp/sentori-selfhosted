//! GET `/v1/push/expo-compat/receipts/{send_id}` — Expo SDK
//! adapter receipt.
//!
//! Wraps `/v1/push/receipts/:send_id` in Expo's `{ data: ... }`
//! envelope shape.

use std::sync::Arc;

use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};
use sentori_ingest_token::IngestContext;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::handlers::sdk::push::receipt::handle as receipt_handle;
use crate::state::AppState;

pub async fn handle(
    Extension(ctx): Extension<IngestContext>,
    State(state): State<Arc<AppState>>,
    Path(delivery_id): Path<Uuid>,
) -> (StatusCode, Json<Value>) {
    // The status code travels with it now: the inner handler answers
    // 404 for an id that does not exist, and an Expo client reading
    // `data.status` off a 200 could not tell that from a delivery
    // that had not been attempted yet.
    let (code, Json(inner)) = receipt_handle(Extension(ctx), State(state), Path(delivery_id)).await;
    (code, Json(json!({ "data": inner })))
}
