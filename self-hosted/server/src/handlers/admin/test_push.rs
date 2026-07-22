//! POST /admin/api/projects/:project_id/push/test
//!
//! Operator clicks this to send a synthetic push to a known
//! device_token (e.g. their dev device). Returns the same shape
//! as /v1/push/send. Saves a round-trip through the SDK.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Extension, Path, State},
    http::StatusCode,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use tracing::warn;
use uuid::Uuid;

use crate::handlers::tenant::guard_project;
use crate::session_mw::SessionContext;
use crate::state::AppState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestPushBody {
    pub device_token_id: Uuid,
    #[serde(default = "default_title")]
    pub title: String,
    #[serde(default = "default_body")]
    pub body: String,
}

fn default_title() -> String {
    "Sentori test".into()
}
fn default_body() -> String {
    "hello from sentori dashboard".into()
}

pub async fn handle(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
    Json(body): Json<TestPushBody>,
) -> (StatusCode, Json<Value>) {
    // Tenant guard: the project must belong to the caller's
    // workspace before we touch its device tokens / queue a send.
    if let Err((code, msg)) = guard_project(&state, ctx.workspace_id, project_id).await {
        return (code, Json(json!({ "error": msg })));
    }
    // Verify the device_token belongs to this project.
    let owns = sqlx::query(
        "SELECT provider FROM device_tokens WHERE id = $1 AND project_id = $2 AND revoked_at IS NULL",
    )
    .bind(body.device_token_id)
    .bind(project_id)
    .fetch_optional(&state.pool)
    .await;
    let provider = match owns {
        Ok(Some(r)) => r.get::<String, _>("provider"),
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "device_token_not_found_or_revoked" })),
            );
        }
        Err(e) => {
            warn!(error = %e, "admin.push.test lookup_failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            );
        }
    };

    let send_id = Uuid::now_v7();
    let result = sqlx::query(
        "INSERT INTO push_sends (id, workspace_id, project_id, token_id, provider, payload, status) \
         VALUES ($1, $2, $3, $4, $5, $6, 'queued') RETURNING id",
    )
    .bind(send_id)
    .bind(ctx.workspace_id.into_uuid())
    .bind(project_id)
    .bind(body.device_token_id)
    .bind(&provider)
    .bind(json!({ "title": body.title, "body": body.body }))
    .fetch_optional(&state.pool)
    .await;
    match result {
        Ok(Some(_)) => (
            StatusCode::ACCEPTED,
            Json(json!({
                "send_id": send_id.to_string(),
                "provider": provider,
                "note": "queued — drained by background push_worker within ~5s",
            })),
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "internal" })),
        ),
    }
}
