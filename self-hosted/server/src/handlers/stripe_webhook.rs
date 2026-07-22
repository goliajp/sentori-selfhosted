//! POST /webhooks/stripe — Stripe webhook receiver.
//!
//! Public (Stripe calls it server-to-server), authenticated by the
//! `Stripe-Signature` HMAC rather than a session. Verifies + dedups
//! the delivery into `stripe_events`, then returns 200 immediately
//! so Stripe stops retrying. The slow apply-to-billing work runs
//! out-of-band in [`crate::billing_worker`].
//!
//! Always 200 on a *verified* event (fresh or duplicate). A bad
//! signature or malformed body is 400 and is NOT persisted.

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use time::OffsetDateTime;

use crate::state::AppState;

pub async fn ingest(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, (StatusCode, String)> {
    let Some(secret) = state.stripe.webhook_secret.as_deref() else {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "stripe webhook disabled (no SENTORI_STRIPE_WEBHOOK_SECRET configured)".into(),
        ));
    };
    let sig = headers
        .get("stripe-signature")
        .and_then(|v| v.to_str().ok())
        .ok_or((
            StatusCode::BAD_REQUEST,
            "missing Stripe-Signature header".into(),
        ))?;
    let now_unix = OffsetDateTime::now_utc().unix_timestamp();
    // fresh vs dedup-hit both answer 200 — the point is to tell
    // Stripe "received", and a duplicate is already recorded.
    crate::stripe::ingest_webhook(&state.pool, &body, sig, secret, now_unix)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    Ok(StatusCode::OK)
}
