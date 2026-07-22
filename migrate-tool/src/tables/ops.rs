//! Ops / monitoring tables — endpoint_check, endpoint_probe,
//! pii_findings, digest_subscriptions, webhook_deliveries.
//!
//! Real legacy names: endpoint_check + endpoint_probe (dst 0029),
//! pii_findings (dst 0030), digest_subscriptions +
//! webhook_deliveries (dst 0026). Only pii_findings has rows in
//! legacy prod; the rest are guarded.

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
    total += endpoint_check(src, dst, dry_run, report).await?;
    total += endpoint_probe(src, dst, dry_run, report).await?;
    total += pii_findings(src, dst, dry_run, report).await?;
    total += digest_subscriptions(src, dst, dry_run, report).await?;
    total += webhook_deliveries(src, dst, dry_run, report).await?;
    Ok(total)
}

async fn endpoint_check(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    if !guard(src, "endpoint_check", report).await? {
        return Ok(0);
    }
    // Legacy + dst 0029: id, project_id, name, target_url,
    // method, interval_sec, assertion_status_codes INT4[],
    // assertion_body_substring, assertion_max_latency_ms,
    // paused, created_by, created_at, updated_at; dst adds
    // workspace_id.
    let rows = sqlx::query(
        "SELECT ec.id, p.org_id AS workspace_id, ec.project_id, ec.name, ec.target_url, \
                ec.method, ec.interval_sec, ec.assertion_status_codes, \
                ec.assertion_body_substring, ec.assertion_max_latency_ms, ec.paused, \
                ec.created_by, ec.created_at, ec.updated_at \
         FROM endpoint_check ec JOIN projects p ON p.id = ec.project_id",
    )
    .fetch_all(src)
    .await?;
    report.note_read("endpoint_check", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO endpoint_check (id, workspace_id, project_id, name, target_url, \
                method, interval_sec, assertion_status_codes, assertion_body_substring, \
                assertion_max_latency_ms, paused, created_by, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14) \
             ON CONFLICT (id) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("id"))
        .bind(r.get::<uuid::Uuid, _>("workspace_id"))
        .bind(r.get::<uuid::Uuid, _>("project_id"))
        .bind(r.get::<String, _>("name"))
        .bind(r.get::<String, _>("target_url"))
        .bind(r.get::<String, _>("method"))
        .bind(r.get::<i32, _>("interval_sec"))
        .bind(r.get::<Vec<i32>, _>("assertion_status_codes"))
        .bind(r.get::<Option<String>, _>("assertion_body_substring"))
        .bind(r.get::<Option<i32>, _>("assertion_max_latency_ms"))
        .bind(r.get::<bool, _>("paused"))
        .bind(r.get::<Option<uuid::Uuid>, _>("created_by"))
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
    report.note_written("endpoint_check", written);
    report.note_skipped("endpoint_check", skipped);
    info!(read = rows.len(), written, skipped, "endpoint_check");
    Ok(written)
}

async fn endpoint_probe(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    if !guard(src, "endpoint_probe", report).await? {
        return Ok(0);
    }
    // Legacy + dst 0029: ts, check_id, status_code, latency_ms,
    // ok, error_kind. Both sides are RANGE-partitioned by ts;
    // the dst DEFAULT partition catches rows outside pre-created
    // daily partitions.
    let rows = sqlx::query(
        "SELECT ts, check_id, status_code, latency_ms, ok, error_kind FROM endpoint_probe",
    )
    .fetch_all(src)
    .await?;
    report.note_read("endpoint_probe", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO endpoint_probe (ts, check_id, status_code, latency_ms, ok, error_kind) \
             VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (check_id, ts) DO NOTHING",
        )
        .bind(r.get::<time::OffsetDateTime, _>("ts"))
        .bind(r.get::<uuid::Uuid, _>("check_id"))
        .bind(r.get::<i32, _>("status_code"))
        .bind(r.get::<i32, _>("latency_ms"))
        .bind(r.get::<bool, _>("ok"))
        .bind(r.get::<Option<String>, _>("error_kind"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("endpoint_probe", written);
    report.note_skipped("endpoint_probe", skipped);
    info!(read = rows.len(), written, skipped, "endpoint_probe");
    Ok(written)
}

async fn pii_findings(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    // Legacy: id, project_id, release, event_id, field_path,
    // pattern_kind, sample, seen_at. Dst 0030 mirrors it and
    // adds workspace_id (derived via projects.org_id).
    let rows = sqlx::query(
        "SELECT pf.id, p.org_id AS workspace_id, pf.project_id, pf.release, pf.event_id, \
                pf.field_path, pf.pattern_kind, pf.sample, pf.seen_at \
         FROM pii_findings pf JOIN projects p ON p.id = pf.project_id",
    )
    .fetch_all(src)
    .await?;
    report.note_read("pii_findings", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO pii_findings (id, workspace_id, project_id, release, event_id, \
                field_path, pattern_kind, sample, seen_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) ON CONFLICT (id) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("id"))
        .bind(r.get::<uuid::Uuid, _>("workspace_id"))
        .bind(r.get::<uuid::Uuid, _>("project_id"))
        .bind(r.get::<String, _>("release"))
        .bind(r.get::<uuid::Uuid, _>("event_id"))
        .bind(r.get::<String, _>("field_path"))
        .bind(r.get::<String, _>("pattern_kind"))
        .bind(r.get::<String, _>("sample"))
        .bind(r.get::<time::OffsetDateTime, _>("seen_at"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("pii_findings", written);
    report.note_skipped("pii_findings", skipped);
    info!(read = rows.len(), written, skipped, "pii_findings");
    Ok(written)
}

async fn digest_subscriptions(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    if !guard(src, "digest_subscriptions", report).await? {
        return Ok(0);
    }
    // Legacy: user_id, org_id, frequency, last_sent_at,
    // created_at. Dst 0026 renames org_id → workspace_id,
    // PK (user_id, workspace_id, frequency).
    let rows = sqlx::query(
        "SELECT user_id, org_id, frequency, last_sent_at, created_at FROM digest_subscriptions",
    )
    .fetch_all(src)
    .await?;
    report.note_read("digest_subscriptions", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO digest_subscriptions (user_id, workspace_id, frequency, last_sent_at, created_at) \
             VALUES ($1, $2, $3, $4, $5) \
             ON CONFLICT (user_id, workspace_id, frequency) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("user_id"))
        .bind(r.get::<uuid::Uuid, _>("org_id"))
        .bind(r.get::<String, _>("frequency"))
        .bind(r.get::<Option<time::OffsetDateTime>, _>("last_sent_at"))
        .bind(r.get::<time::OffsetDateTime, _>("created_at"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("digest_subscriptions", written);
    report.note_skipped("digest_subscriptions", skipped);
    info!(read = rows.len(), written, skipped, "digest_subscriptions");
    Ok(written)
}

async fn webhook_deliveries(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    if !guard(src, "webhook_deliveries", report).await? {
        return Ok(0);
    }
    // Legacy + dst 0026 (identical): id, rule_id, payload,
    // target_url, secret, attempt, next_attempt_at, last_status,
    // last_error, status, created_at, delivered_at.
    let rows = sqlx::query(
        "SELECT id, rule_id, payload, target_url, secret, attempt, next_attempt_at, \
                last_status, last_error, status, created_at, delivered_at \
         FROM webhook_deliveries",
    )
    .fetch_all(src)
    .await?;
    report.note_read("webhook_deliveries", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO webhook_deliveries (id, rule_id, payload, target_url, secret, \
                attempt, next_attempt_at, last_status, last_error, status, created_at, \
                delivered_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12) \
             ON CONFLICT (id) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("id"))
        .bind(r.get::<uuid::Uuid, _>("rule_id"))
        .bind(r.get::<Value, _>("payload"))
        .bind(r.get::<String, _>("target_url"))
        .bind(r.get::<String, _>("secret"))
        .bind(r.get::<i32, _>("attempt"))
        .bind(r.get::<time::OffsetDateTime, _>("next_attempt_at"))
        .bind(r.get::<Option<i32>, _>("last_status"))
        .bind(r.get::<Option<String>, _>("last_error"))
        .bind(r.get::<String, _>("status"))
        .bind(r.get::<time::OffsetDateTime, _>("created_at"))
        .bind(r.get::<Option<time::OffsetDateTime>, _>("delivered_at"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("webhook_deliveries", written);
    report.note_skipped("webhook_deliveries", skipped);
    info!(read = rows.len(), written, skipped, "webhook_deliveries");
    Ok(written)
}
