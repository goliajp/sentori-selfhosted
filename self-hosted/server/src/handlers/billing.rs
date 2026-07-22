//! Self-serve billing — the caller's own workspace subscription.
//!
//! - `GET /admin/api/billing` — plan, status, usage, and which
//!   upgrade paths exist.
//! - `POST /admin/api/billing/checkout` — start a Stripe Checkout for
//!   a plan (Owner/Admin).
//! - `POST /admin/api/billing/portal` — open the Stripe Billing Portal
//!   (Owner/Admin).
//!
//! Session-gated (mounted under `admin_routes`). Mutating routes
//! additionally require `can_manage_workspace` (Owner/Admin) — a
//! plain member can view billing but not change it. Stripe stays
//! the source of truth: these endpoints only *start* a hosted
//! flow; the resulting plan change lands via the webhook worker.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Extension, State};
use axum::http::StatusCode;
use sentori_billing::{
    BillingError, CounterKind, Plan, PlanStatus, WorkspaceBilling, effective_plan, period_key,
};
use serde::Deserialize;
use serde_json::{Value, json};
use time::OffsetDateTime;

use crate::session_mw::SessionContext;
use crate::state::AppState;

/// `GET /admin/api/billing` — the billing panel payload for the
/// caller's active workspace.
pub async fn get(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let billing = state.billing_for(ctx.workspace_id);
    let row: Option<WorkspaceBilling> = match billing.get().await {
        Ok(b) => Some(b),
        Err(BillingError::NotInitialised) => None,
        Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    };
    let plan = row.as_ref().map_or(Plan::Free, |b| b.plan);
    let status = row.as_ref().map_or(PlanStatus::Active, |b| b.status);
    let has_customer = row.as_ref().is_some_and(|b| b.stripe_customer_id.is_some());
    let period_end = row.as_ref().and_then(|b| b.current_period_end);

    let limits = effective_plan(plan, status).limits();
    let now = OffsetDateTime::now_utc();
    let period = period_key(now);
    let usage_rows = billing
        .workspace_usage(&period)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let (mut ev, mut sp, mut rp) = ((0i64, 0i64), (0i64, 0i64), (0i64, 0i64));
    for (kind, count, dropped) in usage_rows {
        match kind {
            CounterKind::Events => ev = (count, dropped),
            CounterKind::Spans => sp = (count, dropped),
            CounterKind::Replays => rp = (count, dropped),
        }
    }

    let cfg = &state.stripe;
    let stripe_enabled = cfg.secret_key.is_some();
    Ok(Json(json!({
        "plan": plan.as_db_str(),
        "status": status.to_string(),
        "effective_plan": effective_plan(plan, status).as_db_str(),
        "current_period_end": period_end,
        "period_yyyymm": period,
        "stripe_enabled": stripe_enabled,
        // Reported separately because the half-configured state is the
        // dangerous one: with a key and a price but no webhook secret,
        // checkout completes and takes the customer's money, and the
        // worker that turns the subscription into a plan change is not
        // running. The plan never moves and nothing says why.
        "webhook_configured": cfg.webhook_secret.is_some(),
        "has_customer": has_customer,
        // Which paid plans this deployment actually sells (a price
        // id is configured). Drives which upgrade buttons render.
        "upgradeable": {
            "pro": cfg.price_for_plan(Plan::Pro).is_some(),
            "enterprise": cfg.price_for_plan(Plan::Enterprise).is_some(),
        },
        "usage": {
            "events":  { "count": ev.0, "dropped": ev.1, "limit": limits.events_monthly },
            "spans":   { "count": sp.0, "dropped": sp.1, "limit": limits.spans_monthly },
            "replays": { "count": rp.0, "dropped": rp.1, "limit": limits.replays_monthly },
        },
    })))
}

#[derive(Deserialize)]
pub struct CheckoutBody {
    /// Target plan: `pro` | `enterprise` (not `free`).
    pub plan: String,
}

/// `POST /admin/api/billing/checkout` — create a Stripe Checkout
/// Session for `plan` and return its hosted `url`.
pub async fn checkout(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Json(body): Json<CheckoutBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    if !ctx.role.can_manage_workspace() {
        return Err((StatusCode::FORBIDDEN, "insufficient_role".into()));
    }
    let plan = Plan::from_db_str(body.plan.trim())
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let cfg = &state.stripe;
    let Some(price_id) = cfg.price_for_plan(plan).map(str::to_string) else {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("plan {plan} is not purchasable (no price configured)"),
        ));
    };

    // Reuse the workspace's Stripe customer if it already has one;
    // otherwise prefill the caller's email for a fresh customer.
    let existing_customer: Option<String> = match state.billing_for(ctx.workspace_id).get().await {
        Ok(b) => b.stripe_customer_id,
        Err(BillingError::NotInitialised) => None,
        Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    };
    let email: Option<String> = sqlx::query_scalar("SELECT email FROM users WHERE id = $1")
        .bind(ctx.user_id.into_uuid())
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let url = crate::stripe::create_checkout_session(
        cfg,
        ctx.workspace_id.into_uuid(),
        existing_customer.as_deref(),
        email.as_deref(),
        &price_id,
    )
    .await
    .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;
    Ok(Json(json!({ "url": url })))
}

/// `POST /admin/api/billing/portal` — open the Stripe Billing
/// Portal for the workspace's customer and return its hosted `url`.
pub async fn portal(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
) -> Result<Json<Value>, (StatusCode, String)> {
    if !ctx.role.can_manage_workspace() {
        return Err((StatusCode::FORBIDDEN, "insufficient_role".into()));
    }
    let customer = match state.billing_for(ctx.workspace_id).get().await {
        Ok(b) => b.stripe_customer_id,
        Err(BillingError::NotInitialised) => None,
        Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    };
    let Some(customer) = customer else {
        return Err((
            StatusCode::BAD_REQUEST,
            "no Stripe customer yet — subscribe first".into(),
        ));
    };
    let url = crate::stripe::create_portal_session(&state.stripe, &customer)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;
    Ok(Json(json!({ "url": url })))
}
