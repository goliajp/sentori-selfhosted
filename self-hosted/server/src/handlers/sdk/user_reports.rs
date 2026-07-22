//! POST `/v1/user-reports` — end-user feedback attached to a
//! crashed event.
//!
//! INSERT into `user_reports` (migration 0021). Optional FK to
//! event / issue lets the dashboard render reports in-context.

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
pub struct UserReportBody {
    #[serde(default)]
    pub event_id: Option<Uuid>,
    #[serde(default)]
    pub issue_id: Option<Uuid>,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

pub async fn handle(
    Extension(ctx): Extension<IngestContext>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<UserReportBody>,
) -> (StatusCode, Json<Value>) {
    if body.title.is_empty() || body.body.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "title + body required" })),
        );
    }
    if body.title.len() > 200 || body.body.len() > 8000 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "title/body too long" })),
        );
    }

    let id = Uuid::now_v7();
    let result = sqlx::query(
        "INSERT INTO user_reports \
         (id, workspace_id, project_id, event_id, issue_id, title, body, email, name) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(id)
    .bind(ctx.workspace_id.into_uuid())
    .bind(ctx.project_id.into_uuid())
    .bind(body.event_id)
    .bind(body.issue_id)
    .bind(&body.title)
    .bind(&body.body)
    .bind(body.email.as_deref())
    .bind(body.name.as_deref())
    .execute(&state.pool)
    .await;

    match result {
        Ok(_) => {
            info!(
                workspace_id = %ctx.workspace_id,
                project_id = %ctx.project_id,
                "sdk.user_reports recorded",
            );
            (
                StatusCode::ACCEPTED,
                Json(json!({ "report_id": id.to_string() })),
            )
        }
        Err(e) => {
            warn!(workspace_id = %ctx.workspace_id, error = %e, "sdk.user_reports db_error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}
