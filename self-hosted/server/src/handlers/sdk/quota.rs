//! Shared quota metering for SDK ingest handlers.
//!
//! Every real SDK ingest path (events, spans, replays) meters
//! against the owning project's plan quota *before* persisting.
//! Over-limit ingests are dropped and the drop is recorded so the
//! usage dashboard can show it. A metering-backend fault fails
//! *open*: we never discard a customer's telemetry because our own
//! `usage_counters` table hiccuped — we under-count that one
//! request and log it instead.
//!
//! Over-quota rejects with `402 Payment Required`, deliberately
//! **not** `429`: the quota is a *monthly* budget, not a short-term
//! rate limit, so it will not recover for days/weeks. Every SDK
//! drops a non-429 4xx silently; the RN SDK by contrast treats 429
//! as retry-after-a-moment and would re-send (with an offline
//! queue) a batch that cannot succeed until the period resets,
//! burning the host app's battery and network. 402 makes deployed
//! clients drop cleanly with no retry churn, no SDK update needed.
//!
//! The limit is driven by the *project's* real workspace plan
//! (see [`sentori_billing::BillingService::check_and_record`]), so
//! passing the shared boot-time-bound `state.billing` handle is
//! correct even in multi-tenant SaaS.

use sentori_billing::{CounterKind, Decision};
use sentori_workspace_identity::ProjectId;
use serde_json::{Value, json};
use time::OffsetDateTime;
use tracing::warn;

use crate::state::AppState;

/// Meter `delta` units of `kind` for `project_id`.
///
/// - `Ok(())` — under/at limit, `delta <= 0`, or a metering fault
///   we chose to ride through (fail-open). The caller proceeds
///   with the ingest.
/// - `Err(body)` — over quota. The drop is already recorded; the
///   caller should return `402 Payment Required` with `body` (a
///   monthly quota is a plan limit, not a retryable rate limit —
///   see the module docs on why not 429).
pub async fn meter(
    state: &AppState,
    project_id: ProjectId,
    kind: CounterKind,
    delta: i64,
    now: OffsetDateTime,
) -> Result<(), Value> {
    // check_and_record rejects delta <= 0; an empty batch is a
    // no-op rather than an error.
    if delta <= 0 {
        return Ok(());
    }
    let decision = match state
        .billing
        .check_and_record(project_id, kind, delta, now)
        .await
    {
        Ok(d) => d,
        Err(e) => {
            // Fail-open: a billing-table fault must not cost the
            // customer their telemetry. Under-count + move on.
            warn!(
                %project_id,
                counter = %kind,
                error = %e,
                "sdk.quota check failed, allowing ingest (fail-open)",
            );
            return Ok(());
        }
    };
    if let Decision::OverLimit {
        current_count,
        limit,
    } = decision
    {
        let _ = state
            .billing
            .record_drop(project_id, kind, delta, now)
            .await;
        return Err(json!({
            "error": "quota_exceeded",
            "counter": kind.to_string(),
            "current": current_count,
            "limit": limit,
        }));
    }
    Ok(())
}
