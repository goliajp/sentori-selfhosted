//! Issue-dashboard tables — watchers, notifications,
//! activity_log, issue_comments, issue_integration_links,
//! issue_user_mutes. Dst 0025 mirrors legacy verbatim except
//! issue_integration_links, which lives in 0011 with a
//! different shape.

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
    total += watchers(src, dst, dry_run, report).await?;
    total += notifications(src, dst, dry_run, report).await?;
    total += activity_log(src, dst, dry_run, report).await?;
    total += issue_comments(src, dst, dry_run, report).await?;
    total += issue_integration_links(src, dst, dry_run, report).await?;
    total += issue_user_mutes(src, dst, dry_run, report).await?;
    Ok(total)
}

async fn watchers(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    // Legacy + dst 0025: (issue_id, user_id, since), PK (issue_id, user_id).
    let rows = sqlx::query("SELECT issue_id, user_id, since FROM watchers")
        .fetch_all(src)
        .await?;
    report.note_read("watchers", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO watchers (issue_id, user_id, since) \
             VALUES ($1, $2, $3) ON CONFLICT (issue_id, user_id) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("issue_id"))
        .bind(r.get::<uuid::Uuid, _>("user_id"))
        .bind(r.get::<time::OffsetDateTime, _>("since"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("watchers", written);
    report.note_skipped("watchers", skipped);
    info!(read = rows.len(), written, skipped, "watchers");
    Ok(written)
}

async fn notifications(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    if !guard(src, "notifications", report).await? {
        return Ok(0);
    }
    // Legacy + dst 0025: id BIGINT, user_id, issue_id, kind,
    // payload, read_at, created_at.
    let rows = sqlx::query(
        "SELECT id, user_id, issue_id, kind, payload, read_at, created_at FROM notifications",
    )
    .fetch_all(src)
    .await?;
    report.note_read("notifications", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO notifications (id, user_id, issue_id, kind, payload, read_at, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT (id) DO NOTHING",
        )
        .bind(r.get::<i64, _>("id"))
        .bind(r.get::<uuid::Uuid, _>("user_id"))
        .bind(r.get::<uuid::Uuid, _>("issue_id"))
        .bind(r.get::<String, _>("kind"))
        .bind(r.get::<Value, _>("payload"))
        .bind(r.get::<Option<time::OffsetDateTime>, _>("read_at"))
        .bind(r.get::<time::OffsetDateTime, _>("created_at"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("notifications", written);
    report.note_skipped("notifications", skipped);
    info!(read = rows.len(), written, skipped, "notifications");
    Ok(written)
}

async fn activity_log(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    if !guard(src, "activity_log", report).await? {
        return Ok(0);
    }
    // Legacy + dst 0025: id BIGINT, issue_id, actor_id, verb, payload, at.
    let rows = sqlx::query("SELECT id, issue_id, actor_id, verb, payload, at FROM activity_log")
        .fetch_all(src)
        .await?;
    report.note_read("activity_log", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO activity_log (id, issue_id, actor_id, verb, payload, at) \
             VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (id) DO NOTHING",
        )
        .bind(r.get::<i64, _>("id"))
        .bind(r.get::<uuid::Uuid, _>("issue_id"))
        .bind(r.get::<Option<uuid::Uuid>, _>("actor_id"))
        .bind(r.get::<String, _>("verb"))
        .bind(r.get::<Value, _>("payload"))
        .bind(r.get::<time::OffsetDateTime, _>("at"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("activity_log", written);
    report.note_skipped("activity_log", skipped);
    info!(read = rows.len(), written, skipped, "activity_log");
    Ok(written)
}

async fn issue_comments(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    if !guard(src, "issue_comments", report).await? {
        return Ok(0);
    }
    // Legacy + dst 0025: id, issue_id, author_id, body, created_at.
    let rows = sqlx::query("SELECT id, issue_id, author_id, body, created_at FROM issue_comments")
        .fetch_all(src)
        .await?;
    report.note_read("issue_comments", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO issue_comments (id, issue_id, author_id, body, created_at) \
             VALUES ($1, $2, $3, $4, $5) ON CONFLICT (id) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("id"))
        .bind(r.get::<uuid::Uuid, _>("issue_id"))
        .bind(r.get::<Option<uuid::Uuid>, _>("author_id"))
        .bind(r.get::<String, _>("body"))
        .bind(r.get::<time::OffsetDateTime, _>("created_at"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("issue_comments", written);
    report.note_skipped("issue_comments", skipped);
    info!(read = rows.len(), written, skipped, "issue_comments");
    Ok(written)
}

async fn issue_integration_links(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    if !guard(src, "issue_integration_links", report).await? {
        return Ok(0);
    }
    // Legacy: issue_id, integration_kind, external_id,
    // external_url (nullable), created_at, external_title,
    // external_status, external_updated_at.
    // Dst is 0011 (NOT 0025): id UUID PK, workspace_id, issue_id,
    // kind, external_id, external_url NOT NULL, created_at — the
    // external_title/status/updated_at columns have no dst home
    // and are dropped; rows with NULL external_url are skipped
    // (dst declares it NOT NULL). id is minted at migration time.
    let rows = sqlx::query(
        "SELECT gen_random_uuid() AS id, p.org_id AS workspace_id, l.issue_id, \
                l.integration_kind, l.external_id, l.external_url, l.created_at \
         FROM issue_integration_links l \
         JOIN issues i ON i.id = l.issue_id \
         JOIN projects p ON p.id = i.project_id",
    )
    .fetch_all(src)
    .await?;
    report.note_read("issue_integration_links", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let external_url: Option<String> = r.get("external_url");
        let Some(external_url) = external_url else {
            skipped += 1;
            continue;
        };
        let res = sqlx::query(
            "INSERT INTO issue_integration_links (id, workspace_id, issue_id, kind, \
                external_id, external_url, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("id"))
        .bind(r.get::<uuid::Uuid, _>("workspace_id"))
        .bind(r.get::<uuid::Uuid, _>("issue_id"))
        .bind(r.get::<String, _>("integration_kind"))
        .bind(r.get::<String, _>("external_id"))
        .bind(external_url)
        .bind(r.get::<time::OffsetDateTime, _>("created_at"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("issue_integration_links", written);
    report.note_skipped("issue_integration_links", skipped);
    info!(read = rows.len(), written, skipped, "issue_integration_links");
    Ok(written)
}

async fn issue_user_mutes(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    if !guard(src, "issue_user_mutes", report).await? {
        return Ok(0);
    }
    // Legacy + dst 0025: (user_id, issue_id, since), PK (user_id, issue_id).
    let rows = sqlx::query("SELECT user_id, issue_id, since FROM issue_user_mutes")
        .fetch_all(src)
        .await?;
    report.note_read("issue_user_mutes", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO issue_user_mutes (user_id, issue_id, since) \
             VALUES ($1, $2, $3) ON CONFLICT (user_id, issue_id) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("user_id"))
        .bind(r.get::<uuid::Uuid, _>("issue_id"))
        .bind(r.get::<time::OffsetDateTime, _>("since"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("issue_user_mutes", written);
    report.note_skipped("issue_user_mutes", skipped);
    info!(read = rows.len(), written, skipped, "issue_user_mutes");
    Ok(written)
}
