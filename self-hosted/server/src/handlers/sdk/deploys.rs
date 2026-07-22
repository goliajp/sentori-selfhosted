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
    /// When the release was deployed. Defaults to now() if absent.
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
    let deploy_at = body.deploy_at.unwrap_or_else(OffsetDateTime::now_utc);

    let result = sqlx::query(
        "INSERT INTO releases (id, workspace_id, project_id, name, deploy_at) \
         VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (project_id, name) DO UPDATE SET deploy_at = EXCLUDED.deploy_at \
         RETURNING id",
    )
    .bind(id)
    .bind(ctx.workspace_id.into_uuid())
    .bind(ctx.project_id.into_uuid())
    .bind(&body.release)
    .bind(deploy_at)
    .fetch_one(&state.pool)
    .await;

    match result {
        Ok(_) => {
            info!(
                workspace_id = %ctx.workspace_id,
                project_id = %ctx.project_id,
                release = %body.release,
                "sdk.deploys recorded",
            );
            (
                StatusCode::ACCEPTED,
                Json(json!({
                    "status": "accepted",
                    "release": body.release,
                    "deploy_at": deploy_at,
                })),
            )
        }
        Err(e) => {
            warn!(workspace_id = %ctx.workspace_id, error = %e, "sdk.deploys db_error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}
