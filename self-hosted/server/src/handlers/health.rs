//! GET /healthz — liveness + DB pool ping + queue depth.

use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Serialize;
use sqlx::Row;

use crate::state::AppState;

#[derive(Serialize)]
pub struct Health {
    status: &'static str,
    db: &'static str,
    version: &'static str,
    pool_size: u32,
    pool_idle: u32,
    push_queued: i64,
    push_failed_24h: i64,
    /// Billing events the worker could not apply. A non-zero value
    /// means someone's subscription changed at Stripe and their plan
    /// here did not — money moved, service did not follow — which is
    /// the worst thing a billing system can do quietly. Until this
    /// existed, such a row was invisible: nothing reads the table.
    billing_failed_24h: i64,
    /// Billing events waiting to be applied. Normally zero, since the
    /// worker drains every 15s; a number that stays up means the queue
    /// is stuck rather than busy.
    billing_pending: i64,
}

pub async fn healthz(State(state): State<Arc<AppState>>) -> (StatusCode, Json<Health>) {
    let db = match sqlx::query("SELECT 1").execute(&state.pool).await {
        Ok(_) => "ok",
        Err(_) => "down",
    };
    let code = if db == "ok" {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    let push_queued: i64 = sqlx::query("SELECT COUNT(*) FROM push_sends WHERE status = 'queued'")
        .fetch_one(&state.pool)
        .await
        .map_or(0, |r| r.get::<i64, _>(0));
    let push_failed_24h: i64 = sqlx::query(
        "SELECT COUNT(*) FROM push_sends WHERE status = 'failed' \
         AND created_at >= now() - interval '24 hours'",
    )
    .fetch_one(&state.pool)
    .await
    .map_or(0, |r| r.get::<i64, _>(0));
    let billing_failed_24h: i64 = sqlx::query(
        "SELECT COUNT(*) FROM stripe_events WHERE processed_state = 'failed' \
         AND received_at >= now() - interval '24 hours'",
    )
    .fetch_one(&state.pool)
    .await
    .map_or(0, |r| r.get::<i64, _>(0));
    let billing_pending: i64 =
        sqlx::query("SELECT COUNT(*) FROM stripe_events WHERE processed_state = 'pending'")
            .fetch_one(&state.pool)
            .await
            .map_or(0, |r| r.get::<i64, _>(0));
    (
        code,
        Json(Health {
            status: if db == "ok" { "ok" } else { "degraded" },
            db,
            version: env!("CARGO_PKG_VERSION"),
            pool_size: state.pool.size(),
            // Bounded by the configured pool size; cannot saturate.
            pool_idle: u32::try_from(state.pool.num_idle()).unwrap_or(u32::MAX),
            push_queued,
            push_failed_24h,
            billing_failed_24h,
            billing_pending,
        }),
    )
}

/// k8s livenessProbe. Returns 200 unconditionally — the process is
/// up. (DB outage shouldn't trigger pod restart; that's readyz' job.)
pub async fn livez() -> StatusCode {
    StatusCode::OK
}

/// k8s readinessProbe. Returns 200 if DB is reachable, 503 otherwise.
/// The kubelet uses this to decide whether to send traffic to this
/// pod — when DB is down we want traffic shifted to peers, not
/// errors served back.
pub async fn readyz(State(state): State<Arc<AppState>>) -> StatusCode {
    match sqlx::query("SELECT 1").execute(&state.pool).await {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::SERVICE_UNAVAILABLE,
    }
}
