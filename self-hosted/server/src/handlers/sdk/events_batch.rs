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
//! heartbeat flood: passes aggregate client-side and
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

use time::OffsetDateTime;
use uuid::Uuid;

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
    /// How many events this SDK discarded since the last envelope —
    /// a full queue, or a spill it could not write. The three
    /// transports have counted this since 3.17.6 and nothing read it,
    /// so a client quietly losing events looked exactly like a client
    /// with nothing to say.
    #[serde(default)]
    pub dropped_events: Option<u32>,
    /// Sessions that ended since the last envelope — the denominator.
    /// Without it the product counts what went wrong and nothing
    /// counts what went right, so "18 errors" has nothing to divide
    /// by and the crash-free rate cannot be computed at all.
    /// Deserialised leniently, one at a time, on purpose. A session
    /// with a malformed id inside a typed `Vec<WireSession>` fails
    /// the *whole body*, which would take a batch of crash reports
    /// down with it — the failure contagion the client contract
    /// forbids. A session we cannot read is skipped; the events it
    /// travelled with still land.
    #[serde(default)]
    pub sessions: Vec<Value>,
}

/// One session, as the SDK's tracker ends it.
///
/// `id` is minted on the device and a resent envelope carries the
/// same one, so the insert is idempotent — a lost response must not
/// turn one session into two and move the rate.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireSession {
    pub id: Uuid,
    pub status: String,
    #[serde(default)]
    pub release: String,
    #[serde(default)]
    pub environment: String,
    #[serde(default)]
    pub platform: String,
    #[serde(with = "time::serde::rfc3339")]
    pub started_at: OffsetDateTime,
    #[serde(default)]
    pub duration_ms: i32,
    /// The salted hash events carry as `userKey`. Both numbers the
    /// console prints — an issue's breadth and the crash-free user rate
    /// — are counted over this column, so they have to be the same
    /// identity space or they are not comparable.
    #[serde(default)]
    pub user_key: Option<String>,
    /// What the field was called before `user_key`, and it was never
    /// populated: the RN SDK hard-wired it to null at session start and
    /// nothing filled it in afterwards. Read as a fallback so an SDK
    /// that does send one is not ignored, but a raw id is not something
    /// this column should be receiving.
    #[serde(default)]
    pub user_id: Option<String>,
}

/// What the table's CHECK accepts. A status we do not know is stored
/// as `ok` rather than refused: the session happened either way, and
/// dropping it would move the denominator — which is worse than
/// losing the detail of how it ended.
const VALID_SESSION_STATUS: [&str; 4] = ["ok", "exited", "errored", "crashed"];

/// What the envelope carries besides events.
///
/// Both before the size check and neither able to fail the batch: a
/// client that dropped events is telling us something whether or not
/// this particular batch is well formed, and a session is the
/// denominator of the crash-free rate — losing one moves a number a
/// release decision is made on.
async fn absorb_telemetry(state: &Arc<AppState>, project_id: Uuid, envelope: &BatchEnvelope) {
    if let Some(n) = envelope.dropped_events {
        state.ingest_counters.client_dropped(u64::from(n));
    }
    store_sessions(state, project_id, &envelope.sessions).await;
}

/// Append the sessions, once each.
///
/// `ON CONFLICT DO NOTHING` on the client-minted id: a retry after a
/// lost response carries the same ids, and counting a session twice
/// moves the rate in the direction that looks better, which is the
/// worst direction for a number to be wrong in.
async fn store_sessions(state: &Arc<AppState>, project_id: Uuid, sessions: &[Value]) {
    if sessions.is_empty() {
        return;
    }
    for raw in sessions {
        let Ok(session) = serde_json::from_value::<WireSession>(raw.clone()) else {
            state.ingest_counters.rejected();
            warn!(project_id = %project_id, "session skipped: unreadable");
            continue;
        };
        let session = &session;
        let status = if VALID_SESSION_STATUS.contains(&session.status.as_str()) {
            session.status.as_str()
        } else {
            // The session happened either way. Dropping it would move
            // the denominator, which is worse than losing the detail
            // of how it ended.
            "ok"
        };
        let platform = if super::events::VALID_PLATFORMS.contains(&session.platform.as_str()) {
            session.platform.as_str()
        } else {
            super::events::UNKNOWN_PLATFORM
        };
        let result = sqlx::query(
            "INSERT INTO sessions \
             (id, project_id, status, release, environment, platform, \
              started_at, duration_ms, user_key) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) \
             ON CONFLICT (id) DO NOTHING",
        )
        .bind(session.id)
        .bind(project_id)
        .bind(status)
        .bind(&session.release)
        .bind(&session.environment)
        .bind(platform)
        .bind(session.started_at)
        .bind(session.duration_ms.max(0))
        .bind(session.user_key.as_deref().or(session.user_id.as_deref()))
        .execute(&state.pool)
        .await;
        if let Err(e) = result {
            warn!(project_id = %project_id, error = %e, "session insert failed");
        }
    }
}

pub async fn handle(
    Extension(ctx): Extension<IngestContext>,
    State(state): State<Arc<AppState>>,
    Json(envelope): Json<BatchEnvelope>,
) -> (StatusCode, Json<Value>) {
    absorb_telemetry(&state, ctx.project_id, &envelope).await;

    if envelope.events.len() > MAX_BATCH {
        state.ingest_counters.rejected();
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
                        state.ingest_counters.accepted();
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
                        state.ingest_counters.rejected();
                        outcomes.push(json!({ "error": "invalid_payload", "detail": msg }));
                    }
                    Err(e) => {
                        state.ingest_counters.failed();
                        warn!(project_id = %ctx.project_id, error = %e, "batch ingest item failed");
                        outcomes.push(json!({ "error": "ingest_failed" }));
                    }
                }
            }
            Err(msg) => {
                state.ingest_counters.rejected();
                outcomes.push(json!({ "error": "invalid_payload", "detail": msg }));
            }
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
