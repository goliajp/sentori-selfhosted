//! `POST /v1/events/validate` — does this body parse, without a token.
//!
//! An agent integrating against an instance it has no credential for
//! could learn nothing about its request until someone handed it a
//! token out of band: auth runs before deserialisation, so every
//! malformed body and every correct one answer `401` identically. The
//! wire format is public — `docs/protocol.md` prints it — so refusing
//! to say whether a body matches it protects nothing and costs the
//! first hour of every integration.
//!
//! What this does NOT do: touch the database, name a project, tell you
//! whether a token would have worked, or store anything. It parses,
//! and it answers.
//!
//! It also answers in JSON. Ingest itself returns axum's `422` as
//! plain text when deserialisation fails, which a client cannot branch
//! on; here the field is named in a JSON body, because the entire
//! point is to be read by a program.

use std::sync::Arc;

use axum::{Json, extract::State, http::StatusCode};
use serde_json::{Value, json};

use super::events::{VALID_PLATFORMS, WireEvent};
use crate::pipeline::Kind;
use crate::state::AppState;

/// serde's message for a missing or mistyped field names it inside
/// backticks. Pulling it out lets a caller branch on the field rather
/// than on English.
fn field_from(msg: &str) -> Option<String> {
    let start = msg.find('`')? + 1;
    let rest = &msg[start..];
    let end = rest.find('`')?;
    Some(rest[..end].to_string())
}

/// The top-level keys `WireEvent` knows. Anything else a caller sends
/// at the top level is silently dropped by serde — which is how
/// `{"error": …}` at the top level parses cleanly and arrives with no
/// error in it. Naming them back is the single most useful thing this
/// endpoint does: the request succeeds, so nothing else ever tells you.
const KNOWN: [&str; 10] = [
    "id",
    "kind",
    "occurredAt",
    "platform",
    "release",
    "environment",
    "name",
    "surface",
    "userKey",
    "payload",
];

pub async fn handle(
    State(_state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let ignored: Vec<String> = body
        .as_object()
        .map(|o| {
            o.keys()
                .filter(|k| !KNOWN.contains(&k.as_str()))
                .cloned()
                .collect()
        })
        .unwrap_or_default();

    let wire: WireEvent = match serde_json::from_value(body) {
        Ok(w) => w,
        Err(e) => {
            let msg = e.to_string();
            let field = field_from(&msg);
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "ok": false,
                    "error": "invalid_payload",
                    "detail": msg,
                    "field": field,
                    "hint": "top-level keys are kind, occurredAt, platform, \
                             release, environment, userKey, name, surface, id. \
                             Everything else about the event goes in `payload`.",
                })),
            );
        }
    };

    // Every kind except `error` is identified by `name`. The check
    // lives in `pipeline.rs`, past a stage that needs a database, so it
    // is restated here rather than shared — and
    // `scripts/check-validate-agrees.mjs` in the e2e is what stops the
    // two from drifting. It earned its place immediately: the first
    // version of this handler omitted these three lines and certified
    // a body ingest answers 400 to.
    let needs_name = match wire.kind {
        Kind::Warn => Some("warn requires name"),
        Kind::Trace | Kind::Assert => Some("trace/assert requires name"),
        Kind::Probe => Some("probe requires ref"),
        Kind::Error => None,
    };
    if let Some(detail) = needs_name
        && wire.name.is_none()
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "ok": false,
                "error": "invalid_payload",
                "detail": detail,
                "field": "name",
            })),
        );
    }

    if !VALID_PLATFORMS.contains(&wire.platform.as_str()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "ok": false,
                "error": "invalid_payload",
                "detail": "platform must be javascript|ios|android",
                "field": "platform",
            })),
        );
    }

    // Echo what was understood, so a caller can see that a field it
    // thought it sent was in fact dropped as unknown.
    (
        StatusCode::OK,
        Json(json!({
            "ok": true,
            "parsed": {
                "kind": wire.kind.as_db_str(),
                "occurredAt": crate::wire_time::rfc3339(wire.occurred_at),
                "platform": wire.platform,
                "release": wire.release,
                "environment": wire.environment,
                "hasPayload": !wire.payload.is_null()
                    && wire.payload != json!({}),
            },
            "ignored": ignored,
            "note": if ignored.is_empty() {
                "parsed only — no token was checked and nothing was stored"
            } else {
                "parsed, but the keys in `ignored` were dropped — they are not \
                 top-level fields. Anything about the event itself belongs \
                 inside `payload`. Nothing was checked or stored."
            },
        })),
    )
}
