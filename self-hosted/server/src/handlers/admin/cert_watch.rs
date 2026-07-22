//! Cert monitor watch-domain admin endpoints:
//!
//! - `POST   /admin/api/projects/:project_id/cert/watches` — add
//! - `DELETE /admin/api/projects/:project_id/cert/watches/:domain`
//!
//! Existing read endpoints (cert/list_watches, cert/list_observations)
//! are in the dashboard handlers — these add the write side.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use sentori_workspace_identity::ProjectId;
use serde::Deserialize;
use serde_json::{Value, json};
use tracing::{info, warn};
use uuid::Uuid;

use crate::state::AppState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddBody {
    pub domain: String,
    #[serde(default)]
    pub added_by: Option<Uuid>,
}

pub async fn add(
    State(state): State<Arc<AppState>>,
    Path(project_id): Path<Uuid>,
    Json(body): Json<AddBody>,
) -> (StatusCode, Json<Value>) {
    let result = sqlx::query(
        "INSERT INTO cert_watch_domains (id, workspace_id, project_id, domain, added_by) \
         SELECT $1, p.workspace_id, $2, $3, $4 FROM projects p WHERE p.id = $2 \
         ON CONFLICT (project_id, domain) DO UPDATE SET added_by = COALESCE(EXCLUDED.added_by, cert_watch_domains.added_by) \
         RETURNING id",
    )
    .bind(Uuid::now_v7())
    .bind(project_id)
    .bind(body.domain.trim().to_ascii_lowercase())
    .bind(body.added_by)
    .fetch_optional(&state.pool)
    .await;

    match result {
        Ok(Some(_)) => {
            info!(%project_id, domain = %body.domain, "admin.cert_watch added");
            (
                StatusCode::CREATED,
                Json(json!({ "domain": body.domain, "status": "watching" })),
            )
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "project_not_found" })),
        ),
        Err(e) => {
            warn!(error = %e, "admin.cert_watch add_failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}

pub async fn remove(
    State(state): State<Arc<AppState>>,
    Path((project_id, domain)): Path<(Uuid, String)>,
) -> StatusCode {
    let normalised = domain.trim().to_ascii_lowercase();
    match sqlx::query("DELETE FROM cert_watch_domains WHERE project_id = $1 AND domain = $2")
        .bind(project_id)
        .bind(&normalised)
        .execute(&state.pool)
        .await
    {
        Ok(_) => {
            info!(%project_id, domain = %normalised, "admin.cert_watch removed");
            StatusCode::NO_CONTENT
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

// Silence unused import on dev when project_id resolves to ProjectId in
// a future commit (currently we use raw Uuid in admin layer).
#[allow(dead_code)]
fn _project_id_use(p: ProjectId) -> ProjectId {
    p
}
