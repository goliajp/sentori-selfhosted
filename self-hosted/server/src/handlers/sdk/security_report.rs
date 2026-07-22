//! POST `/v1/security:report` — trust score inputs (security_events).
//!
//! INSERT into `security_events` (migration 0020). Feeds the
//! /v1/security/score endpoint + dashboard risk view.

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
pub struct SecurityReportBody {
    pub kind: String,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub install_id: Option<String>,
    #[serde(default)]
    pub release: Option<String>,
    #[serde(default)]
    pub environment: Option<String>,
    #[serde(default)]
    pub server_name: Option<String>,
    #[serde(default = "Value::default")]
    pub data: Value,
    #[serde(with = "time::serde::rfc3339")]
    pub occurred_at: OffsetDateTime,
}

pub async fn handle(
    Extension(ctx): Extension<IngestContext>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<SecurityReportBody>,
) -> (StatusCode, Json<Value>) {
    if body.kind.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "kind required" })),
        );
    }
    let id = Uuid::now_v7();
    let result = sqlx::query(
        "INSERT INTO security_events \
         (id, workspace_id, project_id, kind, user_id, install_id, release, environment, server_name, data, occurred_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
    )
    .bind(id)
    .bind(ctx.workspace_id.into_uuid())
    .bind(ctx.project_id.into_uuid())
    .bind(&body.kind)
    .bind(body.user_id.as_deref())
    .bind(body.install_id.as_deref())
    .bind(body.release.as_deref())
    .bind(body.environment.as_deref())
    .bind(body.server_name.as_deref())
    .bind(&body.data)
    .bind(body.occurred_at)
    .execute(&state.pool)
    .await;

    match result {
        Ok(_) => {
            info!(
                workspace_id = %ctx.workspace_id,
                project_id = %ctx.project_id,
                kind = %body.kind,
                "sdk.security_report recorded",
            );
            (
                StatusCode::ACCEPTED,
                Json(json!({ "event_id": id.to_string() })),
            )
        }
        Err(e) => {
            warn!(workspace_id = %ctx.workspace_id, error = %e, "sdk.security_report db_error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}
