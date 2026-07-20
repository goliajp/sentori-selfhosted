//! SaaS billing tables — workspace_billing, billing_subscriptions,
//! billing_invoices, billing_seat_log.
//!
//! Only meaningful on a SaaS deployment cutover; self-hosted
//! has no billing rows in the source DB.

use anyhow::Result;
use serde_json::Value;
use sqlx::{PgPool, Row};
use tracing::info;

use crate::report::Report;

use super::dashboard::guard;

pub async fn migrate(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    let mut total = 0u64;
    total += workspace_billing(src, dst, dry_run, report).await?;
    total += billing_subscriptions(src, dst, dry_run, report).await?;
    total += billing_invoices(src, dst, dry_run, report).await?;
    total += billing_seat_log(src, dst, dry_run, report).await?;
    Ok(total)
}

async fn workspace_billing(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    if !guard(src, "workspace_billing", report).await? {
        return Ok(0);
    }
    let rows = sqlx::query(
        "SELECT id, org_id AS workspace_id, plan, status, stripe_customer_id, \
                stripe_subscription_id, current_period_end, seats, created_at, updated_at \
         FROM workspace_billing",
    )
    .fetch_all(src)
    .await?;
    report.note_read("workspace_billing", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO workspace_billing (id, workspace_id, plan, status, stripe_customer_id, \
                stripe_subscription_id, current_period_end, seats, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) \
             ON CONFLICT (workspace_id) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("id"))
        .bind(r.get::<uuid::Uuid, _>("workspace_id"))
        .bind(r.get::<String, _>("plan"))
        .bind(r.get::<String, _>("status"))
        .bind(r.try_get::<Option<String>, _>("stripe_customer_id").ok().flatten())
        .bind(r.try_get::<Option<String>, _>("stripe_subscription_id").ok().flatten())
        .bind(r.try_get::<Option<time::OffsetDateTime>, _>("current_period_end").ok().flatten())
        .bind(r.try_get::<i32, _>("seats").unwrap_or(1))
        .bind(r.get::<time::OffsetDateTime, _>("created_at"))
        .bind(r.get::<time::OffsetDateTime, _>("updated_at"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("workspace_billing", written);
    report.note_skipped("workspace_billing", skipped);
    info!(read = rows.len(), written, skipped, "workspace_billing");
    Ok(written)
}

async fn billing_subscriptions(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    if !guard(src, "billing_subscriptions", report).await? {
        return Ok(0);
    }
    let rows = sqlx::query(
        "SELECT id, org_id AS workspace_id, stripe_subscription_id, status, plan, \
                price_cents, current_period_start, current_period_end, cancel_at_period_end, \
                created_at, updated_at \
         FROM billing_subscriptions",
    )
    .fetch_all(src)
    .await?;
    report.note_read("billing_subscriptions", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO billing_subscriptions (id, workspace_id, stripe_subscription_id, status, \
                plan, price_cents, current_period_start, current_period_end, cancel_at_period_end, \
                created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) ON CONFLICT (id) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("id"))
        .bind(r.get::<uuid::Uuid, _>("workspace_id"))
        .bind(r.get::<String, _>("stripe_subscription_id"))
        .bind(r.get::<String, _>("status"))
        .bind(r.get::<String, _>("plan"))
        .bind(r.try_get::<i64, _>("price_cents").unwrap_or(0))
        .bind(r.try_get::<Option<time::OffsetDateTime>, _>("current_period_start").ok().flatten())
        .bind(r.try_get::<Option<time::OffsetDateTime>, _>("current_period_end").ok().flatten())
        .bind(r.try_get::<bool, _>("cancel_at_period_end").unwrap_or(false))
        .bind(r.get::<time::OffsetDateTime, _>("created_at"))
        .bind(r.get::<time::OffsetDateTime, _>("updated_at"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("billing_subscriptions", written);
    report.note_skipped("billing_subscriptions", skipped);
    info!(read = rows.len(), written, skipped, "billing_subscriptions");
    Ok(written)
}

async fn billing_invoices(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    if !guard(src, "billing_invoices", report).await? {
        return Ok(0);
    }
    let rows = sqlx::query(
        "SELECT id, org_id AS workspace_id, stripe_invoice_id, status, amount_cents, \
                currency, period_start, period_end, paid_at, raw, created_at \
         FROM billing_invoices",
    )
    .fetch_all(src)
    .await?;
    report.note_read("billing_invoices", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO billing_invoices (id, workspace_id, stripe_invoice_id, status, amount_cents, \
                currency, period_start, period_end, paid_at, raw, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) ON CONFLICT (id) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("id"))
        .bind(r.get::<uuid::Uuid, _>("workspace_id"))
        .bind(r.get::<String, _>("stripe_invoice_id"))
        .bind(r.get::<String, _>("status"))
        .bind(r.get::<i64, _>("amount_cents"))
        .bind(r.try_get::<String, _>("currency").unwrap_or_else(|_| "usd".into()))
        .bind(r.try_get::<Option<time::OffsetDateTime>, _>("period_start").ok().flatten())
        .bind(r.try_get::<Option<time::OffsetDateTime>, _>("period_end").ok().flatten())
        .bind(r.try_get::<Option<time::OffsetDateTime>, _>("paid_at").ok().flatten())
        .bind(r.try_get::<Value, _>("raw").unwrap_or(Value::Null))
        .bind(r.get::<time::OffsetDateTime, _>("created_at"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("billing_invoices", written);
    report.note_skipped("billing_invoices", skipped);
    info!(read = rows.len(), written, skipped, "billing_invoices");
    Ok(written)
}

async fn billing_seat_log(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    if !guard(src, "billing_seat_log", report).await? {
        return Ok(0);
    }
    let rows = sqlx::query(
        "SELECT id, org_id AS workspace_id, delta, reason, actor_user_id, created_at \
         FROM billing_seat_log",
    )
    .fetch_all(src)
    .await?;
    report.note_read("billing_seat_log", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO billing_seat_log (id, workspace_id, delta, reason, actor_user_id, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (id) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("id"))
        .bind(r.get::<uuid::Uuid, _>("workspace_id"))
        .bind(r.get::<i32, _>("delta"))
        .bind(r.try_get::<Option<String>, _>("reason").ok().flatten())
        .bind(r.try_get::<Option<uuid::Uuid>, _>("actor_user_id").ok().flatten())
        .bind(r.get::<time::OffsetDateTime, _>("created_at"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("billing_seat_log", written);
    report.note_skipped("billing_seat_log", skipped);
    info!(read = rows.len(), written, skipped, "billing_seat_log");
    Ok(written)
}
