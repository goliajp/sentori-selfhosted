//! POST `/v1/push/expo-compat/send` — Expo SDK adapter.
//!
//! Delegates to the same send pipeline as `/v1/push/send`. The
//! Expo wire format gets adapted: `to` → `native_tokens`,
//! rest of body → `payload`.

use std::sync::Arc;

use axum::{Extension, Json, extract::State, http::StatusCode};
use sentori_ingest_token::IngestContext;
use serde_json::{Value, json};

use crate::handlers::sdk::push::send::{SendBody, handle as send_handle};
use crate::state::AppState;

pub async fn handle(
    Extension(ctx): Extension<IngestContext>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<Value>,
) -> (StatusCode, Json<Value>) {
    // Same capability as /v1/push/send, reached through the Expo
    // compatibility door. Gating one and not the other would gate
    // nothing.
    if let Err((code, body)) = crate::handlers::sdk::require_admin_token(&ctx) {
        return (code, body);
    }

    let items: Vec<Value> = if let Some(arr) = payload.as_array() {
        arr.clone()
    } else {
        vec![payload]
    };

    let mut all_send_ids: Vec<Value> = Vec::new();
    let mut all_queued = 0u32;
    for item in items {
        let to = item
            .get("to")
            .map(|v| {
                if let Some(a) = v.as_array() {
                    a.iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect::<Vec<_>>()
                } else if let Some(s) = v.as_str() {
                    vec![s.to_string()]
                } else {
                    vec![]
                }
            })
            .unwrap_or_default();

        let body_obj = SendBody {
            token_ids: vec![],
            native_tokens: to,
            topic: None,
            app_user_id: None,
            payload: item.clone(),
            idempotency_key: item
                .get("idempotencyKey")
                .and_then(|v| v.as_str())
                .map(String::from),
            campaign_id: None,
            template_id: None,
        };

        let (status, resp) =
            send_handle(Extension(ctx), State(state.clone()), Json(body_obj)).await;
        if status == StatusCode::ACCEPTED {
            if let Some(send_ids) = resp.get("send_ids").and_then(|v| v.as_array()) {
                all_send_ids.extend(send_ids.clone());
            }
            if let Some(queued) = resp.get("queued").and_then(serde_json::Value::as_u64) {
                all_queued += u32::try_from(queued).unwrap_or(u32::MAX);
            }
        }
    }

    let data: Vec<Value> = all_send_ids
        .iter()
        .map(|id| json!({ "status": "ok", "id": id }))
        .collect();
    (
        StatusCode::ACCEPTED,
        Json(json!({ "data": data, "queued": all_queued })),
    )
}
