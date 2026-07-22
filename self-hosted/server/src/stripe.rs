//! Stripe integration — config, webhook ingest, and the two
//! self-serve billing round-trips (Checkout + Portal).
//!
//! Cement-tier glue: it knows the Sentori billing domain (plans,
//! workspaces) AND talks to a specific external vendor. The pure
//! HMAC signature check lives in the `stripe-webhook-verify` stone;
//! everything commercial (price ids, plan mapping, API shapes)
//! lives here where it can churn without touching a stone.
//!
//! All commercial parameters come from the environment — this file
//! hard-codes no price ids, amounts, or product names.
//!
//! - `SENTORI_STRIPE_SECRET_KEY` — `sk_…`, used as Bearer for the
//!   REST API.
//! - `SENTORI_STRIPE_WEBHOOK_SECRET` — `whsec_…`, HMAC key for inbound
//!   webhook verification.
//! - `SENTORI_STRIPE_PRICE_PRO` — `price_…` for the Pro plan.
//! - `SENTORI_STRIPE_PRICE_ENTERPRISE` — `price_…` for Enterprise
//!   (optional; usually sales-led).
//! - `SENTORI_PUBLIC_URL` — dashboard origin for the Checkout / Portal
//!   return URLs.

use sentori_billing::Plan;
use sentori_stripe_webhook_verify::{Tolerance, verify};
use sqlx::PgPool;
use uuid::Uuid;

const STRIPE_API_BASE: &str = "https://api.stripe.com";

/// Pinned rather than floating on the account's default. Stripe rolls
/// the account default forward, and a version bump can reshape response
/// bodies — an unpinned integration finds out in production, on
/// someone's upgrade click. Move this deliberately, with the changelog
/// open.
const STRIPE_API_VERSION: &str = "2026-06-24.dahlia";

/// Attempts for one logical Stripe write, including the first.
///
/// A customer is waiting on the other end of these calls, so the
/// budget is small: three tries with the backoff below adds at most
/// ~900 ms before giving up. Stripe's own guidance is to retry 5xx,
/// 429 and transport failures; 4xx are our bug or the customer's and
/// retrying them just repeats the same rejection.
const STRIPE_MAX_ATTEMPTS: u32 = 3;

/// Env-driven Stripe configuration. Cloned into every handler that
/// needs it (all fields are small owned strings).
#[derive(Clone, Debug, Default)]
pub struct StripeConfig {
    pub secret_key: Option<String>,
    pub webhook_secret: Option<String>,
    pub price_pro: Option<String>,
    pub price_enterprise: Option<String>,
    /// Dashboard origin, e.g. `https://sentori.golia.jp`. Checkout
    /// success/cancel + Portal return URLs are built off it.
    pub public_url: String,
}

impl StripeConfig {
    /// Read every parameter from the environment. Absent keys leave
    /// their `Option` `None`, which disables the corresponding path
    /// (a deployment with no `secret_key` simply has no self-serve
    /// billing — the endpoints answer 503).
    #[must_use]
    pub fn from_env() -> Self {
        let env = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
        Self {
            secret_key: env("SENTORI_STRIPE_SECRET_KEY"),
            webhook_secret: env("SENTORI_STRIPE_WEBHOOK_SECRET"),
            price_pro: env("SENTORI_STRIPE_PRICE_PRO"),
            price_enterprise: env("SENTORI_STRIPE_PRICE_ENTERPRISE"),
            public_url: env("SENTORI_PUBLIC_URL")
                .unwrap_or_else(|| "https://sentori.golia.jp".to_string()),
        }
    }

    /// The Stripe price id configured for `plan`, if any. `Free`
    /// has no price (it is the absence of a subscription).
    #[must_use]
    pub fn price_for_plan(&self, plan: Plan) -> Option<&str> {
        match plan {
            Plan::Free => None,
            Plan::Pro => self.price_pro.as_deref(),
            Plan::Enterprise => self.price_enterprise.as_deref(),
        }
    }

    /// Reverse map: which plan a Stripe price id sells. Used by the
    /// webhook worker to translate a subscription's price back into
    /// a Sentori plan. Unknown prices return `None` (worker marks
    /// the event failed rather than guessing).
    #[must_use]
    pub fn plan_for_price(&self, price_id: &str) -> Option<Plan> {
        if self.price_pro.as_deref() == Some(price_id) {
            Some(Plan::Pro)
        } else if self.price_enterprise.as_deref() == Some(price_id) {
            Some(Plan::Enterprise)
        } else {
            None
        }
    }
}

/// Verify + persist one Stripe webhook delivery into the
/// `stripe_events` ledger. Returns `Ok(true)` when newly recorded,
/// `Ok(false)` when the event id was already seen (dedup hit — the
/// caller still answers 200 so Stripe stops retrying).
///
/// Ported from the retired `sentori-saas-control` binary; the
/// billing control plane now lives in `sentori-server` against the
/// shared DB (migration 0034).
///
/// # Errors
///
/// - Signature verification failure (caller responds 400; the row
///   is NOT persisted on a bad signature).
/// - JSON / DB errors bubble up.
pub async fn ingest_webhook(
    pool: &PgPool,
    body: &[u8],
    sig_header: &str,
    secret: &str,
    now_unix: i64,
) -> anyhow::Result<bool> {
    verify(
        secret.as_bytes(),
        sig_header,
        body,
        now_unix,
        Tolerance::default(),
    )
    .map_err(|e| anyhow::anyhow!("stripe sig verify: {e}"))?;

    let payload: serde_json::Value = serde_json::from_slice(body)
        .map_err(|e| anyhow::anyhow!("malformed Stripe payload JSON: {e}"))?;
    let stripe_event_id = payload
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Stripe event payload missing `id`"))?;
    let event_type = payload.get("type").and_then(|v| v.as_str()).unwrap_or("");

    let inserted: Option<(Uuid,)> = sqlx::query_as(
        r"
        INSERT INTO stripe_events (id, stripe_event_id, event_type, payload)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (stripe_event_id) DO NOTHING
        RETURNING id
        ",
    )
    .bind(Uuid::now_v7())
    .bind(stripe_event_id)
    .bind(event_type)
    .bind(&payload)
    .fetch_optional(pool)
    .await?;
    Ok(inserted.is_some())
}

/// Create a Stripe Checkout Session for a subscription and return
/// its hosted `url` (the caller 302s / hands it to the browser).
///
/// `client_reference_id = workspace_id` is the thread that lets the
/// `checkout.session.completed` webhook map the payment back to a
/// Sentori workspace. When the workspace already has a Stripe
/// customer we pass it so the subscription attaches to the existing
/// customer; otherwise Stripe creates one (surfaced later via the
/// webhook's `customer` field).
///
/// # Errors
///
/// Network / non-2xx Stripe responses, or a response missing `url`.
pub async fn create_checkout_session(
    cfg: &StripeConfig,
    workspace_id: Uuid,
    customer_id: Option<&str>,
    customer_email: Option<&str>,
    price_id: &str,
) -> anyhow::Result<String> {
    let secret = cfg
        .secret_key
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("SENTORI_STRIPE_SECRET_KEY not configured"))?;
    let ws = workspace_id.to_string();
    let success_url = format!("{}/settings/billing?checkout=success", cfg.public_url);
    let cancel_url = format!("{}/settings/billing?checkout=cancel", cfg.public_url);

    let mut form: Vec<(&str, &str)> = vec![
        ("mode", "subscription"),
        ("line_items[0][price]", price_id),
        ("line_items[0][quantity]", "1"),
        ("client_reference_id", ws.as_str()),
        ("success_url", success_url.as_str()),
        ("cancel_url", cancel_url.as_str()),
        // Mirror the workspace id into subscription metadata too, so
        // later subscription.* events (which carry no
        // client_reference_id) can still be mapped back.
        ("subscription_data[metadata][workspace_id]", ws.as_str()),
    ];
    // Reuse an existing customer when we have one, else let Stripe
    // create it and prefill the email.
    if let Some(cid) = customer_id {
        form.push(("customer", cid));
    } else if let Some(email) = customer_email {
        form.push(("customer_email", email));
    }

    let json = stripe_post(
        secret,
        "/v1/checkout/sessions",
        &form,
        &Uuid::now_v7().to_string(),
        "checkout session",
    )
    .await?;
    json.get("url")
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| anyhow::anyhow!("Stripe checkout session response missing `url`"))
}

/// Create a Billing Portal session for an existing customer and
/// return its hosted `url`. The Portal is where the customer
/// updates card / cancels / views invoices — Stripe hosts the UI.
///
/// # Errors
///
/// Network / non-2xx Stripe responses, or a response missing `url`.
pub async fn create_portal_session(
    cfg: &StripeConfig,
    customer_id: &str,
) -> anyhow::Result<String> {
    let secret = cfg
        .secret_key
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("SENTORI_STRIPE_SECRET_KEY not configured"))?;
    let return_url = format!("{}/settings/billing", cfg.public_url);
    let form: Vec<(&str, &str)> = vec![("customer", customer_id), ("return_url", &return_url)];

    let json = stripe_post(
        secret,
        "/v1/billing_portal/sessions",
        &form,
        &Uuid::now_v7().to_string(),
        "billing portal session",
    )
    .await?;
    json.get("url")
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| anyhow::anyhow!("Stripe portal session response missing `url`"))
}

/// Whether a Stripe HTTP status is worth sending again.
///
/// 4xx other than 429 are our bug or the customer's — a declined card,
/// a price that does not exist, a malformed request. Retrying them
/// repeats the same rejection three times and triples the time the
/// customer spends looking at a spinner before the same failure.
fn is_retryable(status: reqwest::StatusCode) -> bool {
    status.is_server_error() || matches!(status.as_u16(), 429)
}

/// POST a form to Stripe, retrying only what is worth retrying.
///
/// `idempotency_key` is generated once by the caller and held across
/// every attempt — that is the whole point. Without it a retry after a
/// timeout can create a second Checkout Session for one click, and we
/// would never know, because the response that would have told us is
/// the one that got lost.
async fn stripe_post(
    secret: &str,
    path: &str,
    form: &[(&str, &str)],
    idempotency_key: &str,
    what: &str,
) -> anyhow::Result<serde_json::Value> {
    let client = reqwest::Client::new();
    let mut last: anyhow::Error = anyhow::anyhow!("Stripe {what}: no attempt was made");

    for attempt in 0..STRIPE_MAX_ATTEMPTS {
        if attempt > 0 {
            // 150ms, 300ms. No jitter: these retries are per-request
            // and user-initiated, so they do not arrive in a
            // synchronised herd the way a cron's would.
            let backoff = std::time::Duration::from_millis(150u64 << (attempt - 1));
            tokio::time::sleep(backoff).await;
        }

        let sent = client
            .post(format!("{STRIPE_API_BASE}{path}"))
            .bearer_auth(secret)
            .header("Stripe-Version", STRIPE_API_VERSION)
            .header("Idempotency-Key", idempotency_key)
            .form(form)
            .send()
            .await;

        match sent {
            Ok(resp) => {
                let status = resp.status();
                if is_retryable(status) {
                    last = parse_stripe_response(resp, what)
                        .await
                        .err()
                        .unwrap_or_else(|| anyhow::anyhow!("Stripe {what} failed ({status})"));
                    continue;
                }
                return parse_stripe_response(resp, what).await;
            }
            // Transport failure: the request may or may not have
            // reached Stripe, which is exactly the case the
            // idempotency key exists for.
            Err(e) => last = anyhow::Error::new(e).context(format!("Stripe {what} request failed")),
        }
    }

    Err(last.context(format!(
        "Stripe {what} failed after {STRIPE_MAX_ATTEMPTS} attempts"
    )))
}

/// Turn a Stripe HTTP response into JSON, surfacing a non-2xx as a
/// readable error (Stripe puts the human message under
/// `error.message`).
async fn parse_stripe_response(
    resp: reqwest::Response,
    what: &str,
) -> anyhow::Result<serde_json::Value> {
    let status = resp.status();
    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
    if !status.is_success() {
        let msg = body
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .unwrap_or("(no error message)");
        anyhow::bail!("Stripe {what} failed ({status}): {msg}");
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::StatusCode;

    /// The classification is the whole retry policy. Getting it wrong
    /// in the permissive direction turns one declined card into three,
    /// and in the strict direction gives up on the outages retrying
    /// exists for.
    #[test]
    fn only_transient_statuses_are_retried() {
        for s in [
            StatusCode::INTERNAL_SERVER_ERROR,
            StatusCode::BAD_GATEWAY,
            StatusCode::SERVICE_UNAVAILABLE,
            StatusCode::GATEWAY_TIMEOUT,
            StatusCode::TOO_MANY_REQUESTS,
        ] {
            assert!(is_retryable(s), "{s} should be retried");
        }

        for s in [
            StatusCode::OK,
            StatusCode::BAD_REQUEST,
            StatusCode::UNAUTHORIZED,
            StatusCode::FORBIDDEN,
            StatusCode::NOT_FOUND,
            StatusCode::CONFLICT,
            // Stripe returns 402 for a declined card. Retrying it just
            // declines the same card again.
            StatusCode::PAYMENT_REQUIRED,
        ] {
            assert!(!is_retryable(s), "{s} should not be retried");
        }
    }

    /// Three attempts at 150ms + 300ms. The comment on
    /// `STRIPE_MAX_ATTEMPTS` promises a customer waits under a second
    /// extra; this is what holds that promise honest.
    #[test]
    fn retry_budget_stays_under_a_second() {
        let total: u64 = (1..STRIPE_MAX_ATTEMPTS).map(|a| 150u64 << (a - 1)).sum();
        assert_eq!(total, 450);
        assert!(total < 1000);
    }
}
