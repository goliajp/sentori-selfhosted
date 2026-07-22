//! POST `/v1/events` — single event ingest (SDK wire-format).
//!
//! Accepts the legacy SDK JSON payload, maps it to v0.1's
//! `event-pipeline::Event` shape, and persists via
//! `IngestService::ingest`. Returns the issue id + whether the
//! issue is new + whether it flipped from resolved to regressed.

use std::sync::Arc;

use axum::{Extension, Json, extract::State, http::StatusCode};
use sentori_billing::CounterKind;
use sentori_event_pipeline::{Event, EventKind, IngestError, MessageLevel, Platform};
use sentori_ingest_token::IngestContext;
use serde_json::{Value, json};
use time::OffsetDateTime;
use tracing::{info, warn};
use uuid::Uuid;

use crate::state::AppState;

pub async fn handle(
    Extension(ctx): Extension<IngestContext>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let mut event = match map_payload(payload) {
        Ok(e) => e,
        Err(msg) => {
            warn!(workspace_id = %ctx.workspace_id, error = %msg, "sdk.events bad_payload");
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "invalid_payload", "detail": msg })),
            );
        }
    };

    // K17 quota: meter one event against the project's plan
    // before persisting. Malformed payloads (rejected above) do
    // not consume quota.
    let now = OffsetDateTime::now_utc();
    if let Err(body) =
        crate::handlers::sdk::quota::meter(&state, ctx.project_id, CounterKind::Events, 1, now)
            .await
    {
        return (StatusCode::PAYMENT_REQUIRED, Json(body));
    }

    // Cloned before the event moves into ingest, like the tick
    // snapshot below. Only the `user` branch is needed, so the rest of
    // the payload is not copied.
    let (symbolicated, payload_for_identity) = crate::symbolicate::prepare(
        &state,
        ctx.project_id.into_uuid(),
        &event.release,
        &mut event.payload,
    )
    .await;

    let event_tick_snapshot = (
        event.kind.as_db_str().to_string(),
        event.release.clone(),
        event.environment.clone(),
        event.platform.as_db_str().to_string(),
        event.timestamp,
    );
    match state.ingest.ingest(ctx.project_id, event).await {
        Ok(outcome) => {
            info!(
                workspace_id = %ctx.workspace_id,
                project_id = %ctx.project_id,
                issue_id = %outcome.issue_id,
                is_new = outcome.is_new_issue,
                regressed = outcome.regressed,
                symbolicated,
                "sdk.events ingested",
            );
            after_ingest(
                &state,
                &ctx,
                &outcome,
                &payload_for_identity,
                event_tick_snapshot,
            )
            .await;
            (
                StatusCode::ACCEPTED,
                Json(json!({
                    "event_id": outcome.event_id.to_string(),
                    "issue_id": outcome.issue_id.to_string(),
                    "is_new_issue": outcome.is_new_issue,
                    "regressed": outcome.regressed,
                })),
            )
        }
        Err(IngestError::ProjectNotFound(_)) => (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "project_not_found" })),
        ),
        Err(IngestError::InvalidEvent(msg)) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "invalid_event", "detail": msg })),
        ),
        Err(e) => {
            warn!(workspace_id = %ctx.workspace_id, error = %e, "sdk.events db_error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}

/// Public alias for use by `events_batch::handle`.
#[inline]
pub(crate) fn map_payload_pub(p: Value) -> Result<Event, String> {
    map_payload(p)
}

/// Map legacy SDK wire JSON to v0.1 `Event`.
fn map_payload(mut p: Value) -> Result<Event, String> {
    let obj = p.as_object_mut().ok_or("expected JSON object")?;

    let id = obj
        .get("id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .unwrap_or_else(Uuid::now_v7);

    let timestamp = obj
        .get("timestamp")
        .and_then(|v| v.as_str())
        .and_then(|s| OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339).ok())
        .unwrap_or_else(OffsetDateTime::now_utc);

    let kind_str = obj
        .get("kind")
        .and_then(|v| v.as_str())
        .ok_or("missing `kind`")?;
    let kind = match kind_str {
        "error" => EventKind::Error,
        "anr" => EventKind::Anr,
        "nearCrash" | "near_crash" => EventKind::NearCrash,
        "message" => EventKind::Message,
        other => return Err(format!("unknown kind: {other}")),
    };

    let platform_str = obj
        .get("platform")
        .and_then(|v| v.as_str())
        .ok_or("missing `platform`")?;
    let platform = match platform_str {
        "javascript" => Platform::Javascript,
        "ios" => Platform::Ios,
        "android" => Platform::Android,
        other => return Err(format!("unknown platform: {other}")),
    };

    let release = obj
        .get("release")
        .and_then(|v| v.as_str())
        .ok_or("missing `release`")?
        .to_string();

    let environment = obj
        .get("environment")
        .and_then(|v| v.as_str())
        .ok_or("missing `environment`")?
        .to_string();

    let error_type = obj
        .get("error")
        .and_then(|v| v.get("type"))
        .and_then(|v| v.as_str())
        .map(String::from);

    // v1.x SDK sends `error.message` (nested) for Exception-shape
    // events. Top-level `message` is the Message-kind body. Fall
    // back so both layouts work.
    let message = obj
        .get("message")
        .and_then(|v| v.as_str())
        .map(String::from)
        .or_else(|| {
            obj.get("error")
                .and_then(|v| v.get("message"))
                .and_then(|v| v.as_str())
                .map(String::from)
        });

    let level = obj
        .get("level")
        .and_then(|v| v.as_str())
        .and_then(|s| match s {
            "fatal" => Some(MessageLevel::Fatal),
            "error" => Some(MessageLevel::Error),
            "warning" => Some(MessageLevel::Warning),
            "info" => Some(MessageLevel::Info),
            "debug" => Some(MessageLevel::Debug),
            _ => None,
        });

    let fingerprint_override = obj
        .get("fingerprint")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|v| v.as_str())
        .map(String::from);

    Ok(Event {
        id,
        timestamp,
        kind,
        platform,
        release,
        environment,
        error_type,
        message,
        level,
        frame: None, // Phase C step 4 — symbolicator integration
        fingerprint_override,
        payload: Value::Object(obj.clone()),
    })
}

/// Everything that happens after an event lands: attribute it, tell the
/// live tail, and let the alert rules look at it.
///
/// Shared by the single and batch paths, which were carrying the same
/// forty lines twice — and drifting, since the batch path had never
/// gained identity attribution.
pub(super) async fn after_ingest(
    state: &Arc<AppState>,
    ctx: &IngestContext,
    outcome: &sentori_event_pipeline::IngestOutcome,
    identity_slice: &serde_json::Value,
    tick: (String, String, String, String, time::OffsetDateTime),
) {
    let ws = ctx.workspace_id.into_uuid();
    // Best effort: an event we cannot attribute is still worth keeping,
    // and the SDK already did the hashing.
    crate::identity_link::record(state, ws, outcome.event_id, identity_slice).await;

    let (kind, release, environment, platform, timestamp) = tick;
    let _ = state.events_bus.send(crate::state::RecentEventTick {
        project_id: ctx.project_id.into_uuid(),
        issue_id: outcome.issue_id,
        event_id: outcome.event_id,
        kind,
        release,
        environment,
        platform,
        timestamp,
    });

    let fire = |trigger| {
        crate::alert_fire::fire_async(
            state.pool.clone(),
            ws,
            ctx.project_id.into_uuid(),
            outcome.issue_id,
            trigger,
        );
    };
    if outcome.is_new_issue {
        fire(crate::alert_fire::TriggerKind::IssueNew);
    } else if outcome.regressed {
        fire(crate::alert_fire::TriggerKind::Regression);
    }
    // event_count rules can fire on any event; the rule itself checks
    // the threshold against issues.event_count.
    fire(crate::alert_fire::TriggerKind::EventCount);
}
