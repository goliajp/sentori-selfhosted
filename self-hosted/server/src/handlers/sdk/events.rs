//! POST `/v1/events` — single-event ingest (v1 wire format).
//!
//! The wire shape is the five-kind protocol (design.md §2/§4); the
//! SDK's `types.ts` mirrors this. Top-level fields are what the
//! server routes on; everything else rides in `payload` untouched
//! (zero-migration SDK additions).
//!
//! ```json
//! {
//!   "id": "0198...",            // client-minted UUIDv7 (optional)
//!   "kind": "error",            // error|warn|trace|assert|probe
//!   "occurredAt": "2026-07-31T…Z",
//!   "platform": "javascript",   // javascript|ios|android
//!   "release": "app@1.2.3+45",
//!   "environment": "prod",
//!   "name": "pay.gateway-retry",// warn/trace/assert name, probe ref
//!   "surface": {"screen": "/checkout", "element": "PayButton"},
//!   "userKey": "ab12…",         // salted identity hash (SDK-side)
//!   "payload": { "error": {…}, "device": {…}, "signals": […] }
//! }
//! ```
//!
//! JS error payloads are symbolicated before fingerprinting so a
//! minified column shift cannot mint a new issue.

use std::sync::Arc;

use axum::{Extension, Json, extract::State, http::StatusCode};
use sentori_ingest_token::IngestContext;
use serde::Deserialize;
use serde_json::{Value, json};
use time::OffsetDateTime;
use tracing::warn;
use uuid::Uuid;

use crate::pipeline::{self, IncomingEvent, Kind};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireEvent {
    #[serde(default)]
    pub id: Option<Uuid>,
    pub kind: Kind,
    #[serde(with = "time::serde::rfc3339")]
    pub occurred_at: OffsetDateTime,
    pub platform: String,
    #[serde(default)]
    pub release: String,
    #[serde(default)]
    pub environment: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub surface: Option<Value>,
    #[serde(default)]
    pub user_key: Option<String>,
    #[serde(default)]
    pub payload: Value,
}

const VALID_PLATFORMS: [&str; 3] = ["javascript", "ios", "android"];

/// Wire → pipeline shape, with symbolication for error payloads.
pub async fn prepare(
    state: &Arc<AppState>,
    project_id: Uuid,
    mut w: WireEvent,
) -> Result<IncomingEvent, &'static str> {
    if !VALID_PLATFORMS.contains(&w.platform.as_str()) {
        return Err("platform must be javascript|ios|android");
    }
    if w.kind == Kind::Error {
        let n = crate::symbolicate::symbolicate_payload(
            &state.pool,
            &state.attachments,
            &state.source_maps,
            project_id,
            &w.release,
            &mut w.payload,
        )
        .await;
        if n > 0 {
            tracing::debug!(frames = n, "symbolicated");
        }
        // Native frames (JVM class.method+line, or iOS addr+image
        // info) go through the proguard / DWARF resolvers.
        if w.platform == "android" || w.platform == "ios" {
            let n = crate::native_symbolicate::symbolicate_native(
                &state.pool,
                &state.attachments,
                project_id,
                &w.release,
                &w.platform,
                &mut w.payload,
            )
            .await;
            if n > 0 {
                tracing::debug!(frames = n, "native symbolicated");
            }
        }
    }
    Ok(IncomingEvent {
        id: w.id.unwrap_or_else(Uuid::now_v7),
        project_id,
        kind: w.kind,
        platform: w.platform,
        occurred_at: w.occurred_at,
        release: w.release,
        environment: w.environment,
        name: w.name,
        surface: w.surface.unwrap_or_else(|| json!({})),
        user_key: w.user_key,
        payload: w.payload,
    })
}

pub async fn handle(
    Extension(ctx): Extension<IngestContext>,
    State(state): State<Arc<AppState>>,
    Json(wire): Json<WireEvent>,
) -> (StatusCode, Json<Value>) {
    let ev = match prepare(&state, ctx.project_id, wire).await {
        Ok(e) => e,
        Err(msg) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "invalid_payload", "detail": msg })),
            );
        }
    };

    let tick = crate::state::RecentEventTick {
        project_id: ctx.project_id,
        issue_id: Uuid::nil(), // filled below
        event_id: ev.id,
        kind: ev.kind.as_db_str().to_string(),
        release: ev.release.clone(),
        environment: ev.environment.clone(),
        platform: ev.platform.clone(),
        timestamp: ev.occurred_at,
    };

    match pipeline::ingest(&state.pool, ev).await {
        Ok(outcome) => {
            let _ = state.events_bus.send(crate::state::RecentEventTick {
                issue_id: outcome.issue_id,
                ..tick
            });
            crate::notify::spawn_issue_notification(
                &state,
                ctx.project_id,
                outcome.issue_id,
                outcome.is_new_issue,
                outcome.regressed,
            );
            (
                StatusCode::ACCEPTED,
                Json(json!({
                    "eventId": outcome.event_id,
                    "issueId": outcome.issue_id,
                    "isNewIssue": outcome.is_new_issue,
                    "regressed": outcome.regressed,
                })),
            )
        }
        Err(pipeline::IngestError::Invalid(msg)) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "invalid_payload", "detail": msg })),
        ),
        Err(e) => {
            warn!(project_id = %ctx.project_id, error = %e, "ingest failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "ingest_failed" })),
            )
        }
    }
}
