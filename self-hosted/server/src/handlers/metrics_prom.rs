//! GET /metrics — Prometheus text-format exposition.
//!
//! Single-flat-list output so any Prom-compatible scraper can read
//! it. Metrics are computed at scrape time (no persistent registry
//! state) — for v0.2's traffic shape this is cheap and avoids
//! middleware bookkeeping.

use std::sync::Arc;

use axum::{
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
};
use sqlx::Row;

use crate::state::AppState;

// Straight-line Prometheus exposition: one block per metric family.
// Length is inherent to the metric count, not to nesting.
#[allow(clippy::too_many_lines)]
pub async fn handle(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let mut out = String::with_capacity(2048);

    // ── build info ──────────────────────────────────────────
    out.push_str("# HELP sentori_build_info Server build metadata.\n");
    out.push_str("# TYPE sentori_build_info gauge\n");
    out.push_str("sentori_build_info{version=\"");
    out.push_str(env!("CARGO_PKG_VERSION"));
    out.push_str("\"} 1\n");

    // ── pool ────────────────────────────────────────────────
    let pool_size = state.pool.size();
    // Connection counts are bounded by the configured pool size, so
    // the conversion cannot saturate.
    let pool_idle = i64::try_from(state.pool.num_idle()).unwrap_or(i64::MAX);
    line(
        &mut out,
        "sentori_db_pool_size",
        "Configured max DB pool size",
        i64::from(pool_size),
    );
    line(
        &mut out,
        "sentori_db_pool_idle",
        "Currently idle DB connections",
        pool_idle,
    );
    line(
        &mut out,
        "sentori_db_pool_in_use",
        "Active (non-idle) DB connections",
        i64::from(pool_size) - pool_idle,
    );

    // ── push queue depth ────────────────────────────────────
    let push_queued = scalar_i64(
        &state.pool,
        "SELECT COUNT(*)::bigint FROM push_sends WHERE status = 'queued'",
    )
    .await;
    let push_failed_24h = scalar_i64(
        &state.pool,
        "SELECT COUNT(*)::bigint FROM push_sends \
         WHERE status = 'failed' AND created_at >= now() - INTERVAL '24 hours'",
    )
    .await;
    let push_sent_24h = scalar_i64(
        &state.pool,
        "SELECT COUNT(*)::bigint FROM push_sends \
         WHERE status = 'sent' AND created_at >= now() - INTERVAL '24 hours'",
    )
    .await;
    line(
        &mut out,
        "sentori_push_queued",
        "Push sends currently queued",
        push_queued,
    );
    line(
        &mut out,
        "sentori_push_failed_24h",
        "Push sends with status=failed in last 24h",
        push_failed_24h,
    );
    line(
        &mut out,
        "sentori_push_sent_24h",
        "Push sends with status=sent in last 24h",
        push_sent_24h,
    );

    // ── ingest volume 24h ───────────────────────────────────
    let events_24h = scalar_i64(
        &state.pool,
        "SELECT COUNT(*)::bigint FROM events \
         WHERE received_at >= now() - INTERVAL '24 hours'",
    )
    .await;
    let issues_open = scalar_i64(
        &state.pool,
        "SELECT COUNT(*)::bigint FROM issues WHERE status = 'unresolved'",
    )
    .await;
    line(
        &mut out,
        "sentori_events_24h",
        "Total events ingested in last 24h",
        events_24h,
    );
    line(
        &mut out,
        "sentori_issues_open",
        "Issues currently in unresolved state",
        issues_open,
    );

    // ── alerts ──────────────────────────────────────────────
    let alerts_enabled = scalar_i64(
        &state.pool,
        "SELECT COUNT(*)::bigint FROM alert_rules \
         WHERE enabled = TRUE AND COALESCE(muted, FALSE) = FALSE",
    )
    .await;
    line(
        &mut out,
        "sentori_alerts_active",
        "Alert rules currently enabled and not muted",
        alerts_enabled,
    );

    // ── sessions ────────────────────────────────────────────
    let sessions_active = scalar_i64(
        &state.pool,
        "SELECT COUNT(*)::bigint FROM sessions \
         WHERE expires_at > now()",
    )
    .await;
    line(
        &mut out,
        "sentori_user_sessions_active",
        "Active dashboard sessions (sessions.expires_at > now())",
        sessions_active,
    );

    let mut headers = HeaderMap::new();
    headers.insert(
        "content-type",
        HeaderValue::from_static("text/plain; version=0.0.4"),
    );
    (StatusCode::OK, headers, out)
}

async fn scalar_i64(pool: &sqlx::PgPool, sql: &str) -> i64 {
    sqlx::query(sql)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .and_then(|r| r.try_get::<i64, _>(0).ok())
        .unwrap_or(0)
}

fn line(out: &mut String, name: &str, help: &str, value: i64) {
    out.push_str("# HELP ");
    out.push_str(name);
    out.push(' ');
    out.push_str(help);
    out.push('\n');
    out.push_str("# TYPE ");
    out.push_str(name);
    out.push_str(" gauge\n");
    out.push_str(name);
    out.push(' ');
    out.push_str(&value.to_string());
    out.push('\n');
}
