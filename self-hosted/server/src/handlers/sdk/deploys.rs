//! POST `/v1/deploys` — release deployment marker.
//!
//! UPSERTs a row in `releases` table (idempotent on
//! `(project_id, name)`). SDK calls this once per release roll-
//! out so the dashboard "Releases" page can list deploy markers,
//! and downstream events can JOIN against `release.id` (via the
//! release name).

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
pub struct DeployBody {
    /// Release identifier (e.g. `myapp@5.3.1`).
    release: String,
    /// When the release was deployed. Accepted on the wire for SDK
    /// compat; ignored — the v1 releases table keys on created_at.
    #[allow(dead_code)]
    #[serde(default, with = "time::serde::rfc3339::option")]
    deploy_at: Option<OffsetDateTime>,
}

pub async fn handle(
    Extension(ctx): Extension<IngestContext>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<DeployBody>,
) -> (StatusCode, Json<Value>) {
    if body.release.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "release required" })),
        );
    }
    let id = Uuid::now_v7();

    let result = sqlx::query(
        "INSERT INTO releases (id, project_id, name) \
         VALUES ($1, $2, $3) \
         ON CONFLICT (project_id, name) DO UPDATE SET name = EXCLUDED.name \
         RETURNING id",
    )
    .bind(id)
    .bind(ctx.project_id)
    .bind(&body.release)
    .fetch_one(&state.pool)
    .await;

    match result {
        Ok(_) => {
            info!(
                project_id = %ctx.project_id,
                release = %body.release,
                "sdk.deploys recorded",
            );
            (
                StatusCode::ACCEPTED,
                Json(json!({
                    "status": "accepted",
                    "release": body.release,
                })),
            )
        }
        Err(e) => {
            warn!(error = %e, "sdk.deploys db_error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}
