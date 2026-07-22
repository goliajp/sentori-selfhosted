//! POST `/v1/track:batch` — analytics events (≤ 500).
//!
//! Batch insert into `track_events` (migration 0019). Each entry
//! carries `name`, optional `user_id` / `route` / `release` /
//! `environment`, free-form `props` JSONB, and `occurredAt`.

use std::sync::Arc;

use axum::{Extension, Json, extract::State, http::StatusCode};
use sentori_ingest_token::IngestContext;
use serde::Deserialize;
use serde_json::{Value, json};
use time::OffsetDateTime;
use tracing::{info, warn};
use uuid::Uuid;

use crate::state::AppState;

const MAX_BATCH_SIZE: usize = 500;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackEvent {
    pub name: String,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub session_id: Option<Uuid>,
    #[serde(default)]
    pub route: Option<String>,
    #[serde(default)]
    pub release: Option<String>,
    #[serde(default)]
    pub environment: Option<String>,
    #[serde(default = "Value::default")]
    pub props: Value,
    #[serde(with = "time::serde::rfc3339")]
    pub occurred_at: OffsetDateTime,
}

pub async fn handle(
    Extension(ctx): Extension<IngestContext>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let arr = if let Some(a) = payload.as_array() {
        a.clone()
    } else if let Some(a) = payload.get("events").and_then(|v| v.as_array()) {
        a.clone()
    } else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "expected array or { events: [...] }" })),
        );
    };

    if arr.len() > MAX_BATCH_SIZE {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "batch too large",
                "max": MAX_BATCH_SIZE,
                "got": arr.len(),
            })),
        );
    }

    let mut accepted = 0u32;
    let mut failed = 0u32;
    for raw in arr {
        let Ok(ev) = serde_json::from_value::<TrackEvent>(raw) else {
            failed += 1;
            continue;
        };
        let id = Uuid::now_v7();
        let result = sqlx::query(
            "INSERT INTO track_events \
             (id, workspace_id, project_id, name, user_id, session_id, route, release, environment, props, occurred_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
        )
        .bind(id)
        .bind(ctx.workspace_id.into_uuid())
        .bind(ctx.project_id.into_uuid())
        .bind(&ev.name)
        .bind(ev.user_id.as_deref())
        .bind(ev.session_id)
        .bind(ev.route.as_deref())
        .bind(ev.release.as_deref())
        .bind(ev.environment.as_deref())
        .bind(&ev.props)
        .bind(ev.occurred_at)
        .execute(&state.pool)
        .await;
        match result {
            Ok(_) => accepted += 1,
            Err(e) => {
                failed += 1;
                warn!(workspace_id = %ctx.workspace_id, error = %e, "sdk.track item_failed");
            }
        }
    }

    info!(
        workspace_id = %ctx.workspace_id,
        project_id = %ctx.project_id,
        accepted, failed,
        "sdk.track processed",
    );

    (
        StatusCode::ACCEPTED,
        Json(json!({ "accepted": accepted, "failed": failed })),
    )
}
