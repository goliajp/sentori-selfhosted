//! POST `/v1/events:batch` — batched events (≤ 100).
//!
//! Each event passes through the same `map_payload + IngestService::ingest`
//! pipeline as `/v1/events`. Per-event failures are tallied in
//! the response so the SDK can decide whether to retry the whole
//! batch (HTTP success with a non-zero `failed`) or fail-hard
//! (HTTP 4xx).

use std::sync::Arc;

use axum::{Extension, Json, extract::State, http::StatusCode};
use sentori_billing::CounterKind;
use sentori_ingest_token::IngestContext;
use serde_json::{Value, json};
use time::OffsetDateTime;
use tracing::{info, warn};

use crate::handlers::sdk::events::map_payload_pub;
use crate::handlers::sdk::quota;
use crate::state::AppState;

const MAX_BATCH_SIZE: usize = 100;

pub async fn handle(
    Extension(ctx): Extension<IngestContext>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<Value>,
) -> (StatusCode, Json<Value>) {
    // Accept either bare array OR `{ events: [...] }`.
    let events = if let Some(arr) = payload.as_array() {
        arr.clone()
    } else if let Some(arr) = payload.get("events").and_then(|v| v.as_array()) {
        arr.clone()
    } else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "expected array or { events: [...] }" })),
        );
    };

    if events.len() > MAX_BATCH_SIZE {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "batch too large",
                "max": MAX_BATCH_SIZE,
                "got": events.len(),
            })),
        );
    }

    // K17 quota: meter the whole batch atomically (delta = batch
    // size, not 1). Over-limit rejects the entire batch with 402
    // (monthly quota — SDKs drop it rather than retry; see quota.rs).
    let now = OffsetDateTime::now_utc();
    let delta = i64::try_from(events.len()).unwrap_or(i64::MAX);
    if let Err(body) = quota::meter(&state, ctx.project_id, CounterKind::Events, delta, now).await {
        return (StatusCode::PAYMENT_REQUIRED, Json(body));
    }

    let mut symbolicated_total = 0usize;

    let mut accepted = 0u32;
    let mut failed = 0u32;
    let mut results: Vec<Value> = Vec::with_capacity(events.len());

    for raw in events {
        let mut event = match map_payload_pub(raw) {
            Ok(e) => e,
            Err(msg) => {
                failed += 1;
                results.push(json!({ "ok": false, "error": "invalid_payload", "detail": msg }));
                continue;
            }
        };

        // Same reason as the single-event path: cloned before the
        // event moves, and only the branch identity needs.
        let event_tick_snapshot = (
            event.kind.as_db_str().to_string(),
            event.release.clone(),
            event.environment.clone(),
            event.platform.as_db_str().to_string(),
            event.timestamp,
        );
        let (symbolicated, payload_for_identity) = crate::symbolicate::prepare(
            &state,
            ctx.project_id.into_uuid(),
            &event.release,
            &mut event.payload,
        )
        .await;

        match state.ingest.ingest(ctx.project_id, event).await {
            Ok(outcome) => {
                accepted += 1;
                symbolicated_total += symbolicated;
                // Same after-ingest work as the single path. It used to
                // be a partial copy here: no live-tail broadcast and no
                // event_count trigger, so a rule that fired on a single
                // event stayed silent for a batched one.
                super::events::after_ingest(
                    &state,
                    &ctx,
                    &outcome,
                    &payload_for_identity,
                    event_tick_snapshot,
                )
                .await;
                results.push(json!({
                    "ok": true,
                    "event_id": outcome.event_id.to_string(),
                    "issue_id": outcome.issue_id.to_string(),
                    "is_new_issue": outcome.is_new_issue,
                    "regressed": outcome.regressed,
                }));
            }
            Err(e) => {
                failed += 1;
                warn!(workspace_id = %ctx.workspace_id, error = %e, "sdk.events_batch item_failed");
                results.push(json!({ "ok": false, "error": e.to_string() }));
            }
        }
    }

    info!(
        workspace_id = %ctx.workspace_id,
        project_id = %ctx.project_id,
        accepted,
        failed,
        symbolicated = symbolicated_total,
        "sdk.events_batch processed",
    );

    (
        StatusCode::ACCEPTED,
        Json(json!({
            "accepted": accepted,
            "failed": failed,
            "results": results,
        })),
    )
}
