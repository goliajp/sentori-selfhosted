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

/// Public so the unauthenticated validator checks the same list. Two
/// copies of this would let `/v1/events/validate` certify a platform
/// ingest rejects, which is worse than having no validator.
pub const VALID_PLATFORMS: [&str; 3] = ["javascript", "ios", "android"];

/// Replace NUL bytes throughout a JSON value, keys included.
///
/// `\u0000` is valid JSON and invalid in a Postgres `text` or `jsonb`.
/// U+FFFD rather than deletion so the corruption stays visible to
/// whoever reads the payload.
pub fn scrub_nuls(v: &mut Value) {
    match v {
        Value::String(s) => {
            if s.contains('\0') {
                *s = s.replace('\0', "\u{fffd}");
            }
        }
        Value::Array(items) => items.iter_mut().for_each(scrub_nuls),
        Value::Object(map) => {
            let dirty: Vec<String> = map.keys().filter(|k| k.contains('\0')).cloned().collect();
            for k in dirty {
                let val = map.remove(&k).unwrap_or(Value::Null);
                map.insert(k.replace('\0', "\u{fffd}"), val);
            }
            map.values_mut().for_each(scrub_nuls);
        }
        _ => {}
    }
}

/// Wire → pipeline shape, with symbolication for error payloads.
pub async fn prepare(
    state: &Arc<AppState>,
    project_id: Uuid,
    mut w: WireEvent,
) -> Result<IncomingEvent, &'static str> {
    if !VALID_PLATFORMS.contains(&w.platform.as_str()) {
        return Err("platform must be javascript|ios|android");
    }

    // Postgres cannot store a NUL in `text` or `jsonb`, so one
    // mis-encoded byte anywhere in the event reached the driver as a
    // plain database error and left as `500 ingest_failed` — which
    // this product's own contract tells the SDK to retry, forever, for
    // a body that can never succeed. Scrubbing rather than refusing:
    // a NUL is an encoding accident in the host app, and losing the
    // crash report over it is the SDK failing the app for something
    // the app did not mean to do.
    scrub_nuls(&mut w.payload);
    for field in [&mut w.release, &mut w.environment] {
        if field.contains('\0') {
            *field = field.replace('\0', "\u{fffd}");
        }
    }
    for field in [&mut w.name, &mut w.user_key] {
        if let Some(v) = field.as_mut()
            && v.contains('\0')
        {
            *v = v.replace('\0', "\u{fffd}");
        }
    }

    // A device with a wrong clock is a real thing, and the report it
    // sends is still worth having — dropping it would be the SDK
    // failing the host app for something the host app did not do. But
    // an `occurredAt` in the future sorts above everything real and
    // stays there, so the queue's first row becomes an artefact of one
    // broken clock. Clamp forward skew to arrival; keep what was
    // claimed, in the payload, so it is diagnosable rather than lost.
    //
    // Backward skew is left alone: an event cached offline for days is
    // ordinary, and its own timestamp is the true one.
    let now = OffsetDateTime::now_utc();
    if w.occurred_at > now + time::Duration::hours(1) {
        let claimed = crate::wire_time::rfc3339(w.occurred_at);
        if let Some(obj) = w.payload.as_object_mut() {
            obj.insert(
                "_clockSkew".to_string(),
                serde_json::json!({
                    "reportedOccurredAt": claimed,
                    "clampedTo": "arrival",
                }),
            );
        }
        w.occurred_at = now;
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
