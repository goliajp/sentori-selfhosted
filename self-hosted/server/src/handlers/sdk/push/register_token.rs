//! POST `/v1/push/tokens` — register device for push.
//!
//! UPSERT into `push_tokens` via push-provider's
//! `DeviceTokenStore::upsert`. Idempotent on (project_id, kind,
//! native_token).

use std::sync::Arc;

use axum::{Extension, Json, extract::State, http::StatusCode};
use sentori_ingest_token::IngestContext;
use sentori_push_provider::ProviderKind;
use serde::Deserialize;
use serde_json::{Value, json};
use tracing::{info, warn};

use crate::state::AppState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterBody {
    /// Provider: `apns` / `fcm` / `webpush` / `hcm` / `mipush`.
    pub kind: String,
    /// Provider-native token (APNs hex, FCM reg id, web sub JSON).
    pub native_token: String,
    /// Optional environment hint (`production` / `sandbox` for APNs).
    #[serde(default)]
    pub env: Option<String>,
    /// App-side user identifier for targeted dispatch.
    #[serde(default)]
    pub app_user_id: Option<String>,
}

pub async fn handle(
    Extension(ctx): Extension<IngestContext>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<RegisterBody>,
) -> (StatusCode, Json<Value>) {
    let Some(kind) = parse_kind(&body.kind) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "invalid_kind", "got": body.kind })),
        );
    };

    // v0.2 canonical store is `device_tokens` (push.send /
    // subscribe_topic / preferences all query it). UPSERT here +
    // RETURNING id so client gets the actual device_tokens.id back.
    let new_id = uuid::Uuid::now_v7();
    let row = sqlx::query(
        "INSERT INTO device_tokens \
         (id, workspace_id, project_id, provider, env, native_token) \
         VALUES ($1, $2, $3, $4, $5, $6) \
         ON CONFLICT (project_id, provider, native_token) DO UPDATE SET \
            env = COALESCE(EXCLUDED.env, device_tokens.env), \
            revoked_at = NULL, \
            last_seen_at = now(), \
            updated_at = now() \
         RETURNING id, (xmax = 0) AS is_new",
    )
    .bind(new_id)
    .bind(ctx.workspace_id.into_uuid())
    .bind(ctx.project_id.into_uuid())
    .bind(&body.kind)
    .bind(body.env.as_deref())
    .bind(&body.native_token)
    .fetch_one(&state.pool)
    .await;

    // Also UPSERT into the push-provider crate's push_tokens for
    // the legacy dispatcher path. Not load-bearing for v0.2; ignore.
    let _ = state
        .push_tokens
        .upsert(
            ctx.project_id,
            kind,
            &body.native_token,
            body.env.as_deref(),
            body.app_user_id.as_deref(),
        )
        .await;

    match row {
        Ok(row) => {
            use sqlx::Row;
            let device_id: uuid::Uuid = row.get("id");
            let is_new: bool = row.try_get("is_new").unwrap_or(true);
            info!(
                workspace_id = %ctx.workspace_id,
                project_id = %ctx.project_id,
                token_id = %device_id,
                is_new,
                "push.register_token upserted",
            );
            (
                StatusCode::ACCEPTED,
                Json(json!({
                    "token_id": device_id.to_string(),
                    "is_new": is_new,
                })),
            )
        }
        Err(e) => {
            warn!(workspace_id = %ctx.workspace_id, error = %e, "push.register_token db_error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}

fn parse_kind(s: &str) -> Option<ProviderKind> {
    match s {
        "apns" => Some(ProviderKind::Apns),
        "fcm" => Some(ProviderKind::Fcm),
        "webpush" => Some(ProviderKind::WebPush),
        "hcm" => Some(ProviderKind::Hcm),
        "mipush" => Some(ProviderKind::MiPush),
        _ => None,
    }
}
