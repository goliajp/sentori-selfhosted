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
    gauge(
        &mut out,
        &state.pool,
        "sentori_push_queued",
        "Push sends currently queued",
        "SELECT COUNT(*)::bigint FROM push_sends WHERE status = 'queued'",
    )
    .await;
    gauge(
        &mut out,
        &state.pool,
        "sentori_push_failed_24h",
        "Push sends with status=failed in last 24h",
        "SELECT COUNT(*)::bigint FROM push_sends \
         WHERE status = 'failed' AND created_at >= now() - INTERVAL '24 hours'",
    )
    .await;
    gauge(
        &mut out,
        &state.pool,
        "sentori_push_sent_24h",
        "Push sends with status=sent in last 24h",
        "SELECT COUNT(*)::bigint FROM push_sends \
         WHERE status = 'sent' AND created_at >= now() - INTERVAL '24 hours'",
    )
    .await;

    // ── ingest volume 24h ───────────────────────────────────
    gauge(
        &mut out,
        &state.pool,
        "sentori_events_24h",
        "Total events ingested in last 24h",
        "SELECT COUNT(*)::bigint FROM events \
         WHERE received_at >= now() - INTERVAL '24 hours'",
    )
    .await;

    // `status = 'open'`. It read `'unresolved'`, which the column's
    // CHECK constraint has never allowed — `('open','resolved',
    // 'ignored')` since 0003_events.sql. That query succeeded and
    // matched nothing, so this gauge has read 0 for the life of the v1
    // schema. Note the shape: unlike the two below it, no error was
    // ever raised, so no amount of error handling would have caught
    // it. Only reading the column's own constraint does.
    gauge(
        &mut out,
        &state.pool,
        "sentori_issues_open",
        "Issues currently in open state",
        "SELECT COUNT(*)::bigint FROM issues WHERE status = 'open'",
    )
    .await;

    // `sentori_alerts_active` stood here and counted `alert_rules`.
    // There is no such table: it belonged to the pre-v1 schema and the
    // v1 rewrite removed it. The query has failed ever since, and the
    // old `scalar_i64` turned every failure into 0 — so the metric read
    // "no alerts are active", which is also what it would read if the
    // feature existed and nothing was firing. A gauge that cannot
    // distinguish "none" from "broken" is worse than no gauge, so it is
    // gone rather than repointed: nothing in v1 is an alert rule.

    // ── sessions ────────────────────────────────────────────
    // `auth_sessions`, not `sessions` — the table has been called that
    // since 0001_identity.sql and this query never matched it.
    gauge(
        &mut out,
        &state.pool,
        "sentori_user_sessions_active",
        "Active dashboard sessions (auth_sessions.expires_at > now())",
        "SELECT COUNT(*)::bigint FROM auth_sessions WHERE expires_at > now()",
    )
    .await;

    let mut headers = HeaderMap::new();
    headers.insert(
        "content-type",
        HeaderValue::from_static("text/plain; version=0.0.4"),
    );
    (StatusCode::OK, headers, out)
}

// `'static`: every caller passes a literal, and sqlx 0.9 makes
// that a type-level fact rather than a convention.
/// Emit one gauge, or emit nothing.
///
/// Nothing is the point. Three of the metrics in this file queried
/// something that could not answer — two named tables the v1 rewrite
/// removed, one a `status` value the column's CHECK has never
/// allowed — and all three published `0`, which is what a healthy
/// counter reads most of the time. Prometheus already has a
/// representation for "not measured": the series is absent. A scraper
/// can alert on absent. It cannot alert on a zero that means nothing.
async fn gauge(out: &mut String, pool: &sqlx::PgPool, name: &str, help: &str, sql: &'static str) {
    if let Some(v) = scalar_i64(pool, sql).await {
        line(out, name, help, v);
    }
}

/// Run a scalar count, or return `None` if it could not be run.
///
/// It used to return `i64` and `unwrap_or(0)`. That is why two of these
/// metrics queried tables that do not exist for the whole life of the
/// v1 schema without anyone noticing: a failed query and an empty table
/// both read `0`, and 0 is exactly what a healthy counter looks like
/// most of the time. Prometheus has a representation for "I could not
/// measure this" — the absence of the series — and it is not the same
/// as zero. Callers omit the line rather than publish a number they did
/// not measure.
async fn scalar_i64(pool: &sqlx::PgPool, sql: &'static str) -> Option<i64> {
    match sqlx::query(sql).fetch_optional(pool).await {
        Ok(row) => row.and_then(|r| r.try_get::<i64, _>(0).ok()),
        Err(e) => {
            tracing::warn!(error = %e, sql, "metrics: scalar query failed");
            None
        }
    }
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
