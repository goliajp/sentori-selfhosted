//! POST `/v1/sessions` — crash-free session ping.
//!
//! INSERTs a row in the `sessions` table (migration 0018).
//! The SDK pings on app close with `status ∈ {ok, errored,
//! crashed, exited}` so the dashboard can render the crash-
//! free-rate timeseries.

use std::sync::Arc;

use axum::{Extension, Json, extract::State, http::StatusCode};
use sentori_ingest_token::IngestContext;
use serde::Deserialize;
use serde_json::{Value, json};
use time::OffsetDateTime;
use tracing::{info, warn};
use uuid::Uuid;

use crate::state::AppState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionBody {
    #[serde(default)]
    pub user_id: Option<String>,
    pub release: String,
    pub environment: String,
    pub status: String,
    #[serde(with = "time::serde::rfc3339")]
    pub started_at: OffsetDateTime,
    #[serde(default)]
    pub duration_ms: i32,
}

pub async fn handle(
    Extension(ctx): Extension<IngestContext>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<SessionBody>,
) -> (StatusCode, Json<Value>) {
    if !matches!(
        body.status.as_str(),
        "ok" | "errored" | "crashed" | "exited"
    ) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "invalid_status", "got": body.status })),
        );
    }
    let id = Uuid::now_v7();
    let result = sqlx::query(
        "INSERT INTO sessions \
         (id, workspace_id, project_id, user_id, release, environment, status, started_at, duration_ms) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(id)
    .bind(ctx.workspace_id.into_uuid())
    .bind(ctx.project_id.into_uuid())
    .bind(body.user_id.as_deref())
    .bind(&body.release)
    .bind(&body.environment)
    .bind(&body.status)
    .bind(body.started_at)
    .bind(body.duration_ms)
    .execute(&state.pool)
    .await;

    match result {
        Ok(_) => {
            info!(
                workspace_id = %ctx.workspace_id,
                project_id = %ctx.project_id,
                status = %body.status,
                "sdk.sessions recorded",
            );
            (
                StatusCode::ACCEPTED,
                Json(json!({ "session_id": id.to_string() })),
            )
        }
        Err(e) => {
            warn!(workspace_id = %ctx.workspace_id, error = %e, "sdk.sessions db_error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}
