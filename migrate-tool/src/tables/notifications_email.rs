//! Email notification tables — notifications_email_log,
//! notification_preferences, digest_runs. Dst 0026 mirrors the
//! legacy shapes verbatim. All three are empty in legacy prod —
//! guarded, with real-shape queries so a future nonzero run
//! works.

use anyhow::Result;
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
    total += email_log(src, dst, dry_run, report).await?;
    total += notification_preferences(src, dst, dry_run, report).await?;
    total += digest_runs(src, dst, dry_run, report).await?;
    Ok(total)
}

async fn email_log(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    if !guard(src, "notifications_email_log", report).await? {
        return Ok(0);
    }
    // Legacy + dst 0026: id BIGINT, notification_id BIGINT
    // (nullable), user_id, recipient_email, status, subject,
    // last_error, created_at, delivered_at.
    let rows = sqlx::query(
        "SELECT id, notification_id, user_id, recipient_email, status, subject, \
                last_error, created_at, delivered_at \
         FROM notifications_email_log",
    )
    .fetch_all(src)
    .await?;
    report.note_read("notifications_email_log", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO notifications_email_log (id, notification_id, user_id, \
                recipient_email, status, subject, last_error, created_at, delivered_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) ON CONFLICT (id) DO NOTHING",
        )
        .bind(r.get::<i64, _>("id"))
        .bind(r.get::<Option<i64>, _>("notification_id"))
        .bind(r.get::<uuid::Uuid, _>("user_id"))
        .bind(r.get::<String, _>("recipient_email"))
        .bind(r.get::<String, _>("status"))
        .bind(r.get::<String, _>("subject"))
        .bind(r.get::<Option<String>, _>("last_error"))
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
    report.note_written("notifications_email_log", written);
    report.note_skipped("notifications_email_log", skipped);
    info!(read = rows.len(), written, skipped, "notifications_email_log");
    Ok(written)
}

async fn notification_preferences(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    if !guard(src, "notification_preferences", report).await? {
        return Ok(0);
    }
    // Legacy + dst 0026: user_id (PK), muted_kinds TEXT[],
    // cadence, channels TEXT[], updated_at.
    let rows = sqlx::query(
        "SELECT user_id, muted_kinds, cadence, channels, updated_at \
         FROM notification_preferences",
    )
    .fetch_all(src)
    .await?;
    report.note_read("notification_preferences", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO notification_preferences (user_id, muted_kinds, cadence, channels, updated_at) \
             VALUES ($1, $2, $3, $4, $5) ON CONFLICT (user_id) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("user_id"))
        .bind(r.get::<Vec<String>, _>("muted_kinds"))
        .bind(r.get::<String, _>("cadence"))
        .bind(r.get::<Vec<String>, _>("channels"))
        .bind(r.get::<time::OffsetDateTime, _>("updated_at"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("notification_preferences", written);
    report.note_skipped("notification_preferences", skipped);
    info!(read = rows.len(), written, skipped, "notification_preferences");
    Ok(written)
}

async fn digest_runs(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    if !guard(src, "digest_runs", report).await? {
        return Ok(0);
    }
    // Legacy + dst 0026: user_id, cadence, last_sent_at;
    // PK (user_id, cadence).
    let rows = sqlx::query("SELECT user_id, cadence, last_sent_at FROM digest_runs")
        .fetch_all(src)
        .await?;
    report.note_read("digest_runs", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO digest_runs (user_id, cadence, last_sent_at) \
             VALUES ($1, $2, $3) ON CONFLICT (user_id, cadence) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("user_id"))
        .bind(r.get::<String, _>("cadence"))
        .bind(r.get::<Option<time::OffsetDateTime>, _>("last_sent_at"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("digest_runs", written);
    report.note_skipped("digest_runs", skipped);
    info!(read = rows.len(), written, skipped, "digest_runs");
    Ok(written)
}
