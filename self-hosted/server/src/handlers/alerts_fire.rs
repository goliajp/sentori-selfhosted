//! POST /v1/alerts/:id/_fire_test
//!
//! Manual trigger for an alert rule's configured channels. Useful
//! after wiring a new webhook URL — operator clicks "Fire test"
//! and verifies the Slack channel / Discord / pager receives the
//! synthetic event.
//!
//! Reads alert_rules.channels (JSON array), dispatches each
//! `webhook` entry via crate::webhook::deliver.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Extension, Path, State},
    http::StatusCode,
};
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

use crate::session_mw::SessionContext;
use crate::state::AppState;

pub async fn fire_test(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(alert_id): Path<Uuid>,
) -> (StatusCode, Json<Value>) {
    // `AND workspace_id` is the tenant guard: fire-testing another
    // workspace's alert (and thus hitting its webhook/Slack) must be
    // a 404, not a cross-tenant trigger.
    let row =
        sqlx::query("SELECT name, channels FROM alert_rules WHERE id = $1 AND workspace_id = $2")
            .bind(alert_id)
            .bind(ctx.workspace_id.into_uuid())
            .fetch_optional(&state.pool)
            .await;
    let (name, channels) = match row {
        Ok(Some(r)) => (r.get::<String, _>("name"), r.get::<Value, _>("channels")),
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "alert_not_found" })),
            );
        }
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            );
        }
    };

    let mut delivered = 0usize;
    let mut errors: Vec<String> = Vec::new();
    let arr = channels.as_array().cloned().unwrap_or_default();
    for ch in &arr {
        let kind = ch.get("kind").and_then(|v| v.as_str()).unwrap_or("");
        if kind != "webhook" && kind != "slack" {
            continue;
        }
        let url = ch.get("url").and_then(|v| v.as_str());
        let secret = ch.get("secret").and_then(|v| v.as_str());
        let Some(url) = url else {
            continue;
        };
        let payload = json!({
            "type": "alert.fire_test",
            "alert_id": alert_id.to_string(),
            "alert_name": name,
            "message": format!("Test fire from Sentori dashboard for alert \"{name}\""),
        });
        match crate::webhook::deliver(url, secret, &payload).await {
            Ok(_) => delivered += 1,
            Err(e) => errors.push(format!("{url}: {e}")),
        }
    }

    crate::notify::audit(
        &state.pool,
        ctx.workspace_id.into_uuid(),
        None,
        None,
        "alert.fire_test",
        Some("alert"),
        Some(&alert_id.to_string()),
        json!({ "delivered": delivered, "errors": errors }),
    )
    .await;

    (
        StatusCode::OK,
        Json(json!({
            "delivered": delivered,
            "errors": errors,
        })),
    )
}
