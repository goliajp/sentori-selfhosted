//! Compatibility for the deployed Insight sender and pre-v3 device registration.
//! Keep legacy wire fields confined to these routes; authorization stays shared.
use std::sync::Arc;

use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};
use sentori_ingest_token::IngestContext;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use super::{register_token, revoke_token, send};
use crate::state::AppState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Message {
    to: String,
    title: Option<String>,
    body: Option<String>,
    data: Option<Value>,
    idempotency_key: Option<String>,
    campaign_id: Option<String>,
    template_id: Option<String>,
}

fn device_id(handle: &str) -> Result<Uuid, uuid::Error> {
    // Historical format_token_handle used exactly ipt_<uuid>; never treat an
    // unrecognized handle as a provider token or broaden it into an audience.
    Uuid::parse_str(handle.strip_prefix("ipt_").unwrap_or(handle))
}

pub async fn send(
    Extension(ctx): Extension<IngestContext>,
    State(state): State<Arc<AppState>>,
    Json(message): Json<Message>,
) -> (StatusCode, Json<Value>) {
    if let Err(error) = crate::handlers::sdk::require_admin_token(&ctx) {
        return error;
    }
    let Ok(id) = device_id(&message.to) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "bad_target"})),
        );
    };
    let body = send::SendBody {
        token_ids: vec![],
        sp_tokens: vec![id],
        native_tokens: vec![],
        topic: None,
        app_user_id: None,
        traits: None,
        audience: None,
        payload: json!({"title": message.title, "body": message.body, "data": message.data}),
        idempotency_key: message.idempotency_key,
        campaign_id: message.campaign_id,
        template_id: message.template_id,
    };
    send::enqueue(ctx, state, body, true).await
}

pub async fn register(
    ctx: Extension<IngestContext>,
    state: State<Arc<AppState>>,
    Json(mut value): Json<Value>,
) -> (StatusCode, Json<Value>) {
    // The oldest SDK called the provider `provider`; pre-v3 used `kind`.
    let old_provider = value.get("provider").is_some();
    if old_provider {
        if value.get("kind").is_some() {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "invalid_kind"})),
            );
        }
        value["kind"] = value["provider"].take();
    }
    let Ok(body) = serde_json::from_value::<register_token::RegisterBody>(value) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "bad_target"})),
        );
    };
    if body.native_token.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "bad_target"})),
        );
    }
    let (status, Json(mut value)) = register_token::handle(ctx, state.clone(), Json(body)).await;
    if status == StatusCode::ACCEPTED {
        let id = value["spToken"].clone();
        let Some(device) = id.as_str().and_then(|s| Uuid::parse_str(s).ok()) else {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal"})),
            );
        };
        value["token_id"] = id.clone();
        value["is_new"] = value["isNew"].clone();
        // Older native callers store this prefixed handle; all three addresses
        // refer to the same project-scoped device row.
        value["id"] = json!(format!("ipt_{}", device.simple()));
        if old_provider {
            let dates = sqlx::query_as::<_, (time::OffsetDateTime, time::OffsetDateTime)>(
                "SELECT created_at, updated_at FROM device_tokens WHERE id=$1 AND project_id=$2",
            )
            .bind(device)
            .bind(ctx.project_id)
            .fetch_one(&state.pool)
            .await;
            let Ok((created, updated)) = dates else {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "internal"})),
                );
            };
            value["createdAt"] = json!(crate::wire_time::rfc3339(created));
            value["updatedAt"] = json!(crate::wire_time::rfc3339(updated));
        }
    }
    (
        if old_provider && status == StatusCode::ACCEPTED {
            StatusCode::OK
        } else {
            status
        },
        Json(value),
    )
}

pub async fn revoke(
    ctx: Extension<IngestContext>,
    state: State<Arc<AppState>>,
    Path(handle): Path<String>,
) -> (StatusCode, Json<Value>) {
    let Ok(id) = device_id(&handle) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "bad_target"})),
        );
    };
    let (status, Json(mut value)) = revoke_token::handle(ctx, state, Path(id.to_string())).await;
    if status == StatusCode::ACCEPTED {
        value["ok"] = json!(value["status"] == "revoked");
    }
    (status, Json(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn historical_handles_are_exact_device_ids() {
        let id = Uuid::now_v7();
        assert_eq!(device_id(&id.to_string()), Ok(id));
        assert_eq!(device_id(&format!("ipt_{id}")), Ok(id));
        assert_eq!(device_id(&format!("ipt_{}", id.simple())), Ok(id));
        for bad in [
            "ipt_abc",
            "",
            "ipt_ipt_00000000-0000-0000-0000-000000000000",
            "provider-token",
        ] {
            assert!(device_id(bad).is_err());
        }
    }
    #[test]
    fn unsupported_legacy_options_are_not_silently_discarded() {
        let payload = json!({"to": Uuid::nil(), "title": "Insight", "body": "Event", "data": {"cameraId":"fixture"}});
        assert!(serde_json::from_value::<Message>(payload.clone()).is_ok());
        let mut unsupported = payload;
        unsupported["sendAt"] = json!("2030-01-01T00:00:00Z");
        assert!(serde_json::from_value::<Message>(unsupported).is_err());
    }
}
