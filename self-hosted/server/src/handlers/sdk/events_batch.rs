//! POST `/v1/events:batch` — the SDK's normal ship path.
//!
//! Envelope:
//!
//! ```json
//! {
//!   "events": [ <WireEvent>, … ],
//!   "assertStats": [
//!     {"name": "pay.token-fresh", "release": "…", "passDelta": 4093, "failDelta": 0}
//!   ]
//! }
//! ```
//!
//! `assertStats` is how assertion liveness ships without a
//! heartbeat flood (design.md §2): passes aggregate client-side and
//! piggyback here; only failures are real events in `events`.
//!
//! Per-event failures don't fail the batch — the SDK gets a
//! per-index outcome list and drops only what was truly rejected.
//! 207-style semantics with a plain 200: the SDK cares about the
//! body, not the status split.

use std::sync::Arc;

use axum::{Extension, Json, extract::State, http::StatusCode};
use sentori_ingest_token::IngestContext;
use serde::Deserialize;
use serde_json::{Value, json};
use tracing::warn;

use super::events::{WireEvent, prepare};
use crate::pipeline;
use crate::state::AppState;

const MAX_BATCH: usize = 200;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchEnvelope {
    #[serde(default)]
    pub events: Vec<WireEvent>,
    #[serde(default)]
    pub assert_stats: Vec<pipeline::AssertStat>,
    /// The integrator's backend health URL, written in
    /// `sentori.init()` and carried on every batch — the server
    /// remembers it per project and probes it (backend_check_worker).
    #[serde(default)]
    pub backend_health_url: Option<String>,
}

pub async fn handle(
    Extension(ctx): Extension<IngestContext>,
    State(state): State<Arc<AppState>>,
    Json(envelope): Json<BatchEnvelope>,
) -> (StatusCode, Json<Value>) {
    if envelope.events.len() > MAX_BATCH {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(json!({
                "error": "batch_too_large",
                "max": MAX_BATCH,
            })),
        );
    }

    if let Some(url) = envelope
        .backend_health_url
        .as_deref()
        .filter(|u| u.len() <= 512 && (u.starts_with("http://") || u.starts_with("https://")))
    {
        // Written only on change — batches arrive every few seconds.
        let r = sqlx::query(
            "UPDATE projects SET backend_health_url = $2 \
             WHERE id = $1 AND backend_health_url IS DISTINCT FROM $2",
        )
        .bind(ctx.project_id)
        .bind(url)
        .execute(&state.pool)
        .await;
        if let Err(e) = r {
            warn!(project_id = %ctx.project_id, error = %e, "backend url update failed");
        }
    }

    if !envelope.assert_stats.is_empty()
        && let Err(e) =
            pipeline::record_assert_stats(&state.pool, ctx.project_id, &envelope.assert_stats).await
    {
        warn!(project_id = %ctx.project_id, error = %e, "assert stats failed");
    }

    let mut outcomes = Vec::with_capacity(envelope.events.len());
    let mut accepted = 0usize;
    for wire in envelope.events {
        match prepare(&state, ctx.project_id, wire).await {
            Ok(ev) => {
                let tick_kind = ev.kind.as_db_str().to_string();
                let tick = crate::state::RecentEventTick {
                    project_id: ctx.project_id,
                    issue_id: uuid::Uuid::nil(),
                    event_id: ev.id,
                    kind: tick_kind,
                    release: ev.release.clone(),
                    environment: ev.environment.clone(),
                    platform: ev.platform.clone(),
                    timestamp: ev.occurred_at,
                };
                match pipeline::ingest(&state.pool, ev).await {
                    Ok(o) => {
                        accepted += 1;
                        let _ = state.events_bus.send(crate::state::RecentEventTick {
                            issue_id: o.issue_id,
                            ..tick
                        });
                        crate::notify::spawn_issue_notification(
                            &state,
                            ctx.project_id,
                            o.issue_id,
                            o.is_new_issue,
                            o.regressed,
                        );
                        outcomes.push(json!({
                            "eventId": o.event_id,
                            "issueId": o.issue_id,
                            "isNewIssue": o.is_new_issue,
                            "regressed": o.regressed,
                        }));
                    }
                    Err(pipeline::IngestError::Invalid(msg)) => {
                        outcomes.push(json!({ "error": "invalid_payload", "detail": msg }));
                    }
                    Err(e) => {
                        warn!(project_id = %ctx.project_id, error = %e, "batch ingest item failed");
                        outcomes.push(json!({ "error": "ingest_failed" }));
                    }
                }
            }
            Err(msg) => outcomes.push(json!({ "error": "invalid_payload", "detail": msg })),
        }
    }

    (
        StatusCode::OK,
        Json(json!({
            "accepted": accepted,
            "outcomes": outcomes,
        })),
    )
}
