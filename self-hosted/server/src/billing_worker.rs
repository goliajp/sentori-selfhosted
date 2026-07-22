//! Background Stripe billing worker.
//!
//! The webhook receiver ([`crate::handlers::stripe_webhook`]) only
//! verifies + persists each delivery into `stripe_events` (fast,
//! so Stripe gets its 200 immediately). This worker does the slow
//! part out-of-band: poll `processed_state = 'pending'` rows oldest
//! first and apply each to the owning workspace's billing row.
//!
//! Event → action mapping (see `apply_event`):
//! - `checkout.session.completed` → link the Stripe customer id to the
//!   workspace.
//! - `customer.subscription.created` / `.updated` → set plan + status
//!   + customer + period end.
//! - `customer.subscription.deleted` → status = canceled (effective
//!   Free).
//! - `invoice.payment_failed` → status = past_due (grace).
//!
//! Rows that hit a permanent mapping error (unknown price, no
//! resolvable workspace) flip to `failed` with a `process_error`
//! for operator triage; transient DB faults leave the row `pending`
//! for the next poll.

use std::time::Duration;

use sentori_billing::{BillingService, PlanStatus};
use sentori_workspace_identity::WorkspaceId;
use sqlx::PgPool;
use time::OffsetDateTime;
use tokio::time::sleep;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::stripe::StripeConfig;

/// Per-event terminal state.
enum Outcome {
    /// Applied — mark `processed`.
    Processed,
    /// Permanent mapping error — mark `failed` with this diagnostic.
    Failed(String),
    /// Transient (DB) fault — leave `pending`, retry next poll.
    Retry(String),
}

/// Spawn the poller. No-op (logged) when Stripe webhooks aren't
/// configured — with no `webhook_secret` nothing ever lands in
/// `stripe_events`, so there is nothing to drain.
pub fn spawn(pool: PgPool, cfg: StripeConfig) {
    if cfg.webhook_secret.is_none() {
        info!("billing worker disabled (no SENTORI_STRIPE_WEBHOOK_SECRET)");
        return;
    }
    let interval = env_interval();
    let batch = env_batch();
    tokio::spawn(async move {
        info!(
            interval_sec = interval.as_secs(),
            batch, "billing worker started"
        );
        loop {
            match drain(&pool, &cfg, batch).await {
                Ok(0) => debug!("billing worker idle"),
                Ok(n) => info!(processed = n, "billing worker batch"),
                Err(e) => warn!(error = %e, "billing worker poll failed"),
            }
            sleep(interval).await;
        }
    });
}

/// Poll + apply one batch. Returns how many rows reached a terminal
/// state (`processed` or `failed`).
async fn drain(pool: &PgPool, cfg: &StripeConfig, batch: usize) -> Result<usize, sqlx::Error> {
    let rows: Vec<(Uuid, String, serde_json::Value)> = sqlx::query_as(
        "SELECT id, event_type, payload FROM stripe_events \
         WHERE processed_state = 'pending' \
         ORDER BY received_at ASC LIMIT $1",
    )
    .bind(i64::try_from(batch).unwrap_or(i64::MAX))
    .fetch_all(pool)
    .await?;

    let mut done = 0usize;
    for (id, event_type, payload) in rows {
        let outcome = apply_event(pool, cfg, &event_type, &payload).await;
        match outcome {
            Outcome::Processed => {
                mark(pool, id, "processed", None).await?;
                done += 1;
            }
            Outcome::Failed(msg) => {
                warn!(event_type, error = %msg, "billing worker event permanently failed");
                mark(pool, id, "failed", Some(&msg)).await?;
                done += 1;
            }
            Outcome::Retry(msg) => {
                // Leave pending; next poll retries.
                warn!(event_type, error = %msg, "billing worker event deferred (transient)");
            }
        }
    }
    Ok(done)
}

async fn mark(pool: &PgPool, id: Uuid, state: &str, err: Option<&str>) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE stripe_events \
         SET processed_state = $2, process_error = $3, processed_at = now() \
         WHERE id = $1",
    )
    .bind(id)
    .bind(state)
    .bind(err)
    .execute(pool)
    .await?;
    Ok(())
}

/// Apply one event to billing. `payload` is the full Stripe event;
/// the resource is at `payload.data.object`.
async fn apply_event(
    pool: &PgPool,
    cfg: &StripeConfig,
    event_type: &str,
    payload: &serde_json::Value,
) -> Outcome {
    let object = &payload["data"]["object"];
    match event_type {
        "checkout.session.completed" => apply_checkout_completed(pool, object).await,
        "customer.subscription.created" | "customer.subscription.updated" => {
            apply_subscription(pool, cfg, object).await
        }
        "customer.subscription.deleted" => {
            apply_status_change(pool, object, PlanStatus::Canceled).await
        }
        "invoice.payment_failed" => apply_status_change(pool, object, PlanStatus::PastDue).await,
        // The recovery half of `invoice.payment_failed`. Without it a
        // workspace that goes past_due and then pays stays past_due
        // until its next subscription.updated — Stripe does send one,
        // but "eventually, via a different event" is not a recovery
        // path, it is a gap that happens to close on its own most of
        // the time.
        "invoice.paid" => apply_status_change(pool, object, PlanStatus::Active).await,
        // Anything we don't model is intentionally a no-op success:
        // Stripe sends many event types we never subscribed logic to.
        other => {
            debug!(
                event_type = other,
                "billing worker ignoring unmodelled event"
            );
            Outcome::Processed
        }
    }
}

/// `checkout.session.completed` — bind the Stripe customer id to the
/// workspace named by `client_reference_id`, so later subscription /
/// invoice events (which only carry `customer`) can be mapped.
async fn apply_checkout_completed(pool: &PgPool, session: &serde_json::Value) -> Outcome {
    let Some(workspace_id) = session
        .get("client_reference_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
    else {
        return Outcome::Failed("checkout.session missing client_reference_id".into());
    };
    let Some(customer) = str_field(session, "customer") else {
        return Outcome::Failed("checkout.session missing customer".into());
    };
    // Upsert the customer linkage; the subscription event that
    // follows sets the actual plan.
    let res = sqlx::query(
        "INSERT INTO workspace_billing (id, workspace_id, plan, status, stripe_customer_id) \
         VALUES ($1, $2, 'free', 'active', $3) \
         ON CONFLICT (workspace_id) DO UPDATE SET \
            stripe_customer_id = EXCLUDED.stripe_customer_id, updated_at = now()",
    )
    .bind(Uuid::now_v7())
    .bind(workspace_id)
    .bind(&customer)
    .execute(pool)
    .await;
    match res {
        Ok(_) => Outcome::Processed,
        Err(e) => Outcome::Retry(format!("link customer: {e}")),
    }
}

/// `customer.subscription.created|updated` — the authoritative
/// plan-setting event. Carries the price (→ plan), status, customer,
/// and current period end.
async fn apply_subscription(pool: &PgPool, cfg: &StripeConfig, sub: &serde_json::Value) -> Outcome {
    let Some(workspace_id) = resolve_workspace(pool, sub).await else {
        return Outcome::Failed("subscription: cannot resolve workspace".into());
    };
    let Some(price_id) = sub["items"]["data"][0]["price"]["id"].as_str() else {
        return Outcome::Failed("subscription missing items[0].price.id".into());
    };
    let Some(plan) = cfg.plan_for_price(price_id) else {
        return Outcome::Failed(format!("no plan configured for price {price_id}"));
    };
    let customer = str_field(sub, "customer");
    // `current_period_end` sat on the subscription in older Stripe
    // API versions; newer ones moved it to the line item. Accept
    // either so period display works regardless of pinned version.
    let period_unix = sub
        .get("current_period_end")
        .and_then(serde_json::Value::as_i64)
        .or_else(|| sub["items"]["data"][0]["current_period_end"].as_i64());
    let period_end = period_unix.and_then(|s| OffsetDateTime::from_unix_timestamp(s).ok());
    let status = sub
        .get("status")
        .and_then(|v| v.as_str())
        .map_or(PlanStatus::Active, map_stripe_status);

    let billing = BillingService::new(pool.clone(), WorkspaceId::from_uuid(workspace_id));
    if let Err(e) = billing
        .set_plan(plan, customer.as_deref(), period_end)
        .await
    {
        return classify(&e, "set_plan");
    }
    if let Err(e) = billing.set_status(status).await {
        return classify(&e, "set_status");
    }
    Outcome::Processed
}

/// Route a billing failure to the outcome that ends.
///
/// Everything used to be `Retry`, which meant a foreign-key violation —
/// a subscription pointing at a workspace that no longer exists — was
/// deferred every 15 seconds for as long as the process lived. Not a
/// slow retry: an unending one, on a row that could never succeed,
/// logged at WARN so it read like a passing blip.
fn classify(e: &sentori_billing::BillingError, what: &str) -> Outcome {
    if e.is_permanent() {
        Outcome::Failed(format!("{what}: {e}"))
    } else {
        Outcome::Retry(format!("{what}: {e}"))
    }
}

/// `customer.subscription.deleted` / `invoice.payment_failed` —
/// status-only transitions.
async fn apply_status_change(
    pool: &PgPool,
    object: &serde_json::Value,
    status: PlanStatus,
) -> Outcome {
    let Some(workspace_id) = resolve_workspace(pool, object).await else {
        return Outcome::Failed("status change: cannot resolve workspace".into());
    };
    let billing = BillingService::new(pool.clone(), WorkspaceId::from_uuid(workspace_id));
    match billing.set_status(status).await {
        Ok(()) => Outcome::Processed,
        Err(e) => classify(&e, "set_status"),
    }
}

/// Resolve the Sentori workspace a Stripe object belongs to. Prefer
/// the `metadata.workspace_id` we stamp onto every subscription;
/// fall back to a lookup by the object's `customer` id (invoices
/// carry no metadata). `invoice.payment_failed`'s object also
/// exposes `subscription` but not its metadata, so the customer
/// lookup is the reliable path there.
async fn resolve_workspace(pool: &PgPool, object: &serde_json::Value) -> Option<Uuid> {
    if let Some(ws) = object["metadata"]["workspace_id"]
        .as_str()
        .and_then(|s| Uuid::parse_str(s).ok())
    {
        return Some(ws);
    }
    let customer = str_field(object, "customer")?;
    let row: Option<(Uuid,)> =
        sqlx::query_as("SELECT workspace_id FROM workspace_billing WHERE stripe_customer_id = $1")
            .bind(&customer)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
    row.map(|(id,)| id)
}

/// A Stripe id field that is delivered either as a bare string
/// (`"cus_123"`) or, when expanded, as an object (`{ "id": … }`).
fn str_field(object: &serde_json::Value, key: &str) -> Option<String> {
    match object.get(key)? {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Object(_) => object[key]["id"].as_str().map(String::from),
        _ => None,
    }
}

/// Map a Stripe subscription status to a Sentori [`PlanStatus`].
/// `incomplete` / `incomplete_expired` / `paused` are treated as
/// past-due (grace) rather than fully canceled — the customer may
/// still complete payment.
fn map_stripe_status(s: &str) -> PlanStatus {
    match s {
        "active" => PlanStatus::Active,
        "trialing" => PlanStatus::Trialing,
        "canceled" => PlanStatus::Canceled,
        "unpaid" => PlanStatus::Unpaid,
        // past_due, incomplete, incomplete_expired, paused, …
        _ => PlanStatus::PastDue,
    }
}

fn env_interval() -> Duration {
    let secs = std::env::var("SENTORI_BILLING_WORKER_INTERVAL_SEC")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(15);
    Duration::from_secs(secs)
}

fn env_batch() -> usize {
    std::env::var("SENTORI_BILLING_WORKER_BATCH")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(50)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn stripe_status_mapping() {
        assert_eq!(map_stripe_status("active"), PlanStatus::Active);
        assert_eq!(map_stripe_status("trialing"), PlanStatus::Trialing);
        assert_eq!(map_stripe_status("canceled"), PlanStatus::Canceled);
        assert_eq!(map_stripe_status("unpaid"), PlanStatus::Unpaid);
        assert_eq!(map_stripe_status("past_due"), PlanStatus::PastDue);
        // Unknown / incomplete → grace, never a silent Active.
        assert_eq!(map_stripe_status("incomplete"), PlanStatus::PastDue);
        assert_eq!(map_stripe_status("paused"), PlanStatus::PastDue);
    }

    #[test]
    fn str_field_handles_bare_and_expanded() {
        let bare = serde_json::json!({ "customer": "cus_123" });
        assert_eq!(str_field(&bare, "customer").as_deref(), Some("cus_123"));

        let expanded = serde_json::json!({ "customer": { "id": "cus_456" } });
        assert_eq!(str_field(&expanded, "customer").as_deref(), Some("cus_456"));

        let missing = serde_json::json!({ "customer": null });
        assert_eq!(str_field(&missing, "customer"), None);
    }
}
