//! GET `/v1/push/expo-compat/receipts/{send_id}` — Expo SDK
//! adapter receipt.
//!
//! Wraps `/v1/push/receipts/:send_id` in Expo's `{ data: ... }`
//! envelope shape.

use std::sync::Arc;

use axum::{
    Extension, Json,
    extract::{Path, State},
};
use sentori_ingest_token::IngestContext;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::handlers::sdk::push::receipt::handle as receipt_handle;
use crate::state::AppState;

pub async fn handle(
    Extension(ctx): Extension<IngestContext>,
    State(state): State<Arc<AppState>>,
    Path(send_id): Path<Uuid>,
) -> Json<Value> {
    let Json(inner) = receipt_handle(Extension(ctx), State(state), Path(send_id)).await;
    Json(json!({ "data": inner }))
}
