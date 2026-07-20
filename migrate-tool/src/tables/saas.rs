//! SaaS-only tables — saasadmin_users, saas_provisioning_log,
//! and anything else legacy saas-control owned.
//!
//! Skipped silently when source schema lacks them (self-hosted
//! cutover never has these rows).

use anyhow::Result;
use serde_json::Value;
use sqlx::{PgPool, Row};
use tracing::info;

use crate::report::Report;

pub async fn migrate(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    let mut total = 0u64;
    total += saasadmin_users(src, dst, dry_run, report).await?;
    total += saas_provisioning_log(src, dst, dry_run, report).await?;
    total += saas_stripe_customers(src, dst, dry_run, report).await?;
    total += saas_stripe_invoices(src, dst, dry_run, report).await?;
    total += saas_org_quotas(src, dst, dry_run, report).await?;
    total += saas_billing_events(src, dst, dry_run, report).await?;
    Ok(total)
}

async fn saas_stripe_customers(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    let Ok(rows) = sqlx::query(
        "SELECT id, workspace_id, stripe_customer_id, email, created_at \
         FROM saas_stripe_customers",
    )
    .fetch_all(src)
    .await
    else {
        report.note_read("saas_stripe_customers", 0);
        return Ok(0);
    };
    report.note_read("saas_stripe_customers", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO saas_stripe_customers (id, workspace_id, stripe_customer_id, email, created_at) \
             VALUES ($1, $2, $3, $4, $5) ON CONFLICT (id) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("id"))
        .bind(r.get::<uuid::Uuid, _>("workspace_id"))
        .bind(r.get::<String, _>("stripe_customer_id"))
        .bind(r.try_get::<Option<String>, _>("email").ok().flatten())
        .bind(r.get::<time::OffsetDateTime, _>("created_at"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("saas_stripe_customers", written);
    report.note_skipped("saas_stripe_customers", skipped);
    info!(read = rows.len(), written, skipped, "saas_stripe_customers");
    Ok(written)
}

async fn saas_stripe_invoices(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    let Ok(rows) = sqlx::query(
        "SELECT id, workspace_id, stripe_invoice_id, amount_cents, currency, status, period_yyyymm, created_at \
         FROM saas_stripe_invoices ORDER BY created_at DESC LIMIT 50000",
    )
    .fetch_all(src)
    .await
    else {
        report.note_read("saas_stripe_invoices", 0);
        return Ok(0);
    };
    report.note_read("saas_stripe_invoices", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO saas_stripe_invoices (id, workspace_id, stripe_invoice_id, amount_cents, currency, status, period_yyyymm, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8) ON CONFLICT (id) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("id"))
        .bind(r.get::<uuid::Uuid, _>("workspace_id"))
        .bind(r.get::<String, _>("stripe_invoice_id"))
        .bind(r.get::<i64, _>("amount_cents"))
        .bind(r.try_get::<String, _>("currency").unwrap_or_else(|_| "JPY".into()))
        .bind(r.get::<String, _>("status"))
        .bind(r.try_get::<Option<String>, _>("period_yyyymm").ok().flatten())
        .bind(r.get::<time::OffsetDateTime, _>("created_at"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("saas_stripe_invoices", written);
    report.note_skipped("saas_stripe_invoices", skipped);
    info!(read = rows.len(), written, skipped, "saas_stripe_invoices");
    Ok(written)
}

async fn saas_org_quotas(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    let Ok(rows) = sqlx::query(
        "SELECT workspace_id, plan, event_cap, attachment_cap_bytes, push_cap, updated_at \
         FROM saas_org_quotas",
    )
    .fetch_all(src)
    .await
    else {
        report.note_read("saas_org_quotas", 0);
        return Ok(0);
    };
    report.note_read("saas_org_quotas", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO saas_org_quotas (workspace_id, plan, event_cap, attachment_cap_bytes, push_cap, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (workspace_id) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("workspace_id"))
        .bind(r.try_get::<String, _>("plan").unwrap_or_else(|_| "free".into()))
        .bind(r.try_get::<i64, _>("event_cap").unwrap_or(100_000))
        .bind(r.try_get::<i64, _>("attachment_cap_bytes").unwrap_or(1_073_741_824))
        .bind(r.try_get::<i64, _>("push_cap").unwrap_or(10_000))
        .bind(r.get::<time::OffsetDateTime, _>("updated_at"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("saas_org_quotas", written);
    report.note_skipped("saas_org_quotas", skipped);
    info!(read = rows.len(), written, skipped, "saas_org_quotas");
    Ok(written)
}

async fn saas_billing_events(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    let Ok(rows) = sqlx::query(
        "SELECT id, workspace_id, event_type, payload, created_at \
         FROM saas_billing_events ORDER BY created_at DESC LIMIT 50000",
    )
    .fetch_all(src)
    .await
    else {
        report.note_read("saas_billing_events", 0);
        return Ok(0);
    };
    report.note_read("saas_billing_events", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO saas_billing_events (id, workspace_id, event_type, payload, created_at) \
             VALUES ($1, $2, $3, $4, $5) ON CONFLICT (id) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("id"))
        .bind(r.get::<uuid::Uuid, _>("workspace_id"))
        .bind(r.get::<String, _>("event_type"))
        .bind(r.try_get::<Value, _>("payload").unwrap_or(Value::Null))
        .bind(r.get::<time::OffsetDateTime, _>("created_at"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("saas_billing_events", written);
    report.note_skipped("saas_billing_events", skipped);
    info!(read = rows.len(), written, skipped, "saas_billing_events");
    Ok(written)
}

async fn saasadmin_users(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    let Ok(rows) = sqlx::query(
        "SELECT id, email, password_hash, role, created_at FROM saasadmin_users",
    )
    .fetch_all(src)
    .await
    else {
        report.note_read("saasadmin_users", 0);
        return Ok(0);
    };
    report.note_read("saasadmin_users", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO saasadmin_users (id, email, password_hash, role, created_at) \
             VALUES ($1, $2, $3, $4, $5) ON CONFLICT (id) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("id"))
        .bind(r.get::<String, _>("email"))
        .bind(r.get::<String, _>("password_hash"))
        .bind(r.try_get::<String, _>("role").unwrap_or_else(|_| "saasadmin".into()))
        .bind(r.get::<time::OffsetDateTime, _>("created_at"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("saasadmin_users", written);
    report.note_skipped("saasadmin_users", skipped);
    info!(read = rows.len(), written, skipped, "saasadmin_users");
    Ok(written)
}

async fn saas_provisioning_log(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    let Ok(rows) = sqlx::query(
        "SELECT id, workspace_id, step, status, details, created_at FROM saas_provisioning_log \
         ORDER BY created_at DESC LIMIT 10000",
    )
    .fetch_all(src)
    .await
    else {
        report.note_read("saas_provisioning_log", 0);
        return Ok(0);
    };
    report.note_read("saas_provisioning_log", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO saas_provisioning_log (id, workspace_id, step, status, details, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (id) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("id"))
        .bind(r.get::<uuid::Uuid, _>("workspace_id"))
        .bind(r.get::<String, _>("step"))
        .bind(r.get::<String, _>("status"))
        .bind(r.try_get::<Value, _>("details").unwrap_or(Value::Null))
        .bind(r.get::<time::OffsetDateTime, _>("created_at"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("saas_provisioning_log", written);
    report.note_skipped("saas_provisioning_log", skipped);
    info!(read = rows.len(), written, skipped, "saas_provisioning_log");
    Ok(written)
}
