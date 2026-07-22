//! POST /v1/events/:project_id — SDK ingest.
//!
//! Minimal v0.1 skeleton — accepts a JSON event body,
//! validates the project, calls K17 billing quota check,
//! then K4 IngestService. Returns 202 + outcome.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use sentori_billing::{CounterKind, Decision};
use sentori_event_pipeline::{Event, Platform};
use sentori_workspace_identity::ProjectId;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::state::AppState;

#[derive(Deserialize)]
pub struct IngestBody {
    pub kind: String, // 'error' | 'message' | …
    pub error_type: String,
    pub message: String,
    pub platform: String, // 'ios' | 'android' | 'web' | …
    #[serde(default = "default_release")]
    pub release: String,
    #[serde(default = "default_environment")]
    pub environment: String,
}

fn default_release() -> String {
    "unknown".into()
}
fn default_environment() -> String {
    "production".into()
}

#[derive(Serialize)]
pub struct IngestResponse {
    pub event_id: Uuid,
    pub issue_id: Uuid,
    pub is_new: bool,
}

pub async fn ingest_event(
    State(state): State<Arc<AppState>>,
    Path(project_id_raw): Path<Uuid>,
    Json(body): Json<IngestBody>,
) -> Result<(StatusCode, Json<IngestResponse>), (StatusCode, String)> {
    let project_id = ProjectId::from_uuid(project_id_raw);
    let now = OffsetDateTime::now_utc();

    // K17 quota check.
    let decision = state
        .billing
        .check_and_record(project_id, CounterKind::Events, 1, now)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("billing: {e}")))?;
    if let Decision::OverLimit {
        current_count,
        limit,
    } = decision
    {
        let _ = state
            .billing
            .record_drop(project_id, CounterKind::Events, 1, now)
            .await;
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            format!("quota exceeded: {current_count}/{limit} events this period"),
        ));
    }

    // K4 ingest.
    let platform = parse_platform(&body.platform);
    let event = Event::exception(
        Uuid::now_v7(),
        now,
        platform,
        &body.release,
        &body.environment,
        &body.error_type,
        &body.message,
    );
    let outcome = state
        .ingest
        .ingest(project_id, event)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("ingest: {e}")))?;
    let _ = body.kind; // reserved for future kind-routing.
    Ok((
        StatusCode::ACCEPTED,
        Json(IngestResponse {
            event_id: outcome.event_id,
            issue_id: outcome.issue_id,
            is_new: outcome.is_new_issue,
        }),
    ))
}

fn parse_platform(s: &str) -> Platform {
    match s.trim().to_ascii_lowercase().as_str() {
        "android" => Platform::Android,
        "web" | "node" | "javascript" | "js" => Platform::Javascript,
        _ => Platform::Ios,
    }
}
