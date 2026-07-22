//! POST /admin/api/webhooks/test
//! { url, secret?, message? } → fires a test webhook payload to
//! the URL (with optional HMAC signature) so operators can verify
//! integrations before wiring a webhook channel into an alert rule.

use std::sync::Arc;

use axum::{Json, extract::State, http::StatusCode};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::state::AppState;

#[derive(Deserialize)]
pub struct TestBody {
    pub url: String,
    #[serde(default)]
    pub secret: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}

pub async fn handle(
    State(_state): State<Arc<AppState>>,
    Json(body): Json<TestBody>,
) -> (StatusCode, Json<Value>) {
    let payload = json!({
        "type": "test",
        "message": body.message.unwrap_or_else(|| "Sentori webhook test".to_string()),
        "source": "sentori-dashboard",
    });
    match crate::webhook::deliver(&body.url, body.secret.as_deref(), &payload).await {
        Ok(status) => (
            StatusCode::OK,
            Json(json!({ "delivered": true, "status": status })),
        ),
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "delivered": false, "error": e.to_string() })),
        ),
    }
}
