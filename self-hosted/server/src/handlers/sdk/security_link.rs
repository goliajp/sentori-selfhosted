//! POST `/v1/security/link` — federated identity upsert (user_federation_links).
//!
//! UPSERT into `user_federation_links` (migration 0020) keyed on
//! `(project_id, provider, subject)`. Used by the SDK after a
//! successful SSO sign-in to correlate sentori's user_id with
//! Auth0 / Cognito / Firebase identities.

use std::sync::Arc;

use axum::{Extension, Json, extract::State, http::StatusCode};
use sentori_ingest_token::IngestContext;
use serde::Deserialize;
use serde_json::{Value, json};
use tracing::{info, warn};
use uuid::Uuid;

use crate::state::AppState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityLinkBody {
    pub provider: String,
    pub subject: String,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub install_id: Option<String>,
}

pub async fn handle(
    Extension(ctx): Extension<IngestContext>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<SecurityLinkBody>,
) -> (StatusCode, Json<Value>) {
    if body.provider.is_empty() || body.subject.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "provider + subject required" })),
        );
    }
    let id = Uuid::now_v7();
    let result = sqlx::query(
        "INSERT INTO user_federation_links \
         (id, workspace_id, project_id, provider, subject, user_id, install_id) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) \
         ON CONFLICT (project_id, provider, subject) DO UPDATE SET \
            user_id = COALESCE(EXCLUDED.user_id, user_federation_links.user_id), \
            install_id = COALESCE(EXCLUDED.install_id, user_federation_links.install_id) \
         RETURNING id",
    )
    .bind(id)
    .bind(ctx.workspace_id.into_uuid())
    .bind(ctx.project_id.into_uuid())
    .bind(&body.provider)
    .bind(&body.subject)
    .bind(body.user_id.as_deref())
    .bind(body.install_id.as_deref())
    .fetch_one(&state.pool)
    .await;

    match result {
        Ok(_) => {
            info!(
                workspace_id = %ctx.workspace_id,
                project_id = %ctx.project_id,
                provider = %body.provider,
                "sdk.security_link upserted",
            );
            (
                StatusCode::ACCEPTED,
                Json(json!({ "link_id": id.to_string() })),
            )
        }
        Err(e) => {
            warn!(workspace_id = %ctx.workspace_id, error = %e, "sdk.security_link db_error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}
