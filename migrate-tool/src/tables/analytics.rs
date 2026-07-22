//! Analytics / security / federation tables — track_events,
//! security_events, user_federation_links, user_reports.

use anyhow::Result;
use serde_json::Value;
use sqlx::{PgPool, Row};
use tracing::info;

use crate::report::Report;

use super::dashboard::guard;

const PAGE: i64 = 2000;

pub async fn migrate(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    let mut total = 0u64;
    total += track_events(src, dst, dry_run, report).await?;
    total += security_events(src, dst, dry_run, report).await?;
    total += federation(src, dst, dry_run, report).await?;
    total += user_reports(src, dst, dry_run, report).await?;
    Ok(total)
}

async fn track_events(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    let mut written = 0u64;
    let mut skipped = 0u64;
    let mut offset: i64 = 0;
    loop {
        let rows = sqlx::query(
            "SELECT te.id, p.org_id AS workspace_id, te.project_id, te.name, te.user_id, \
                    te.session_id, te.route, te.release, te.environment, te.props, \
                    te.occurred_at, te.received_at \
             FROM track_events te JOIN projects p ON p.id = te.project_id \
             ORDER BY te.received_at LIMIT $1 OFFSET $2",
        )
        .bind(PAGE)
        .bind(offset)
        .fetch_all(src)
        .await?;
        if rows.is_empty() {
            break;
        }
        report.note_read("track_events", rows.len() as u64);
        for r in &rows {
            if dry_run {
                continue;
            }
            let res = sqlx::query(
                "INSERT INTO track_events (id, workspace_id, project_id, name, user_id, \
                    session_id, route, release, environment, props, occurred_at, received_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12) \
                 ON CONFLICT (id) DO NOTHING",
            )
            .bind(r.get::<uuid::Uuid, _>("id"))
            .bind(r.get::<uuid::Uuid, _>("workspace_id"))
            .bind(r.get::<uuid::Uuid, _>("project_id"))
            .bind(r.get::<String, _>("name"))
            .bind(r.try_get::<Option<String>, _>("user_id").ok().flatten())
            .bind(r.try_get::<Option<uuid::Uuid>, _>("session_id").ok().flatten())
            .bind(r.try_get::<Option<String>, _>("route").ok().flatten())
            .bind(r.try_get::<Option<String>, _>("release").ok().flatten())
            .bind(r.try_get::<Option<String>, _>("environment").ok().flatten())
            .bind(r.try_get::<Value, _>("props").unwrap_or(Value::Null))
            .bind(r.get::<time::OffsetDateTime, _>("occurred_at"))
            .bind(r.get::<time::OffsetDateTime, _>("received_at"))
            .execute(dst)
            .await?;
            if res.rows_affected() > 0 {
                written += 1;
            } else {
                skipped += 1;
            }
        }
        info!(offset, page = rows.len(), written, skipped, "track_events page");
        offset += PAGE;
        if rows.len() < PAGE as usize {
            break;
        }
    }
    report.note_written("track_events", written);
    report.note_skipped("track_events", skipped);
    Ok(written)
}

async fn security_events(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    if !guard(src, "security_events", report).await? {
        return Ok(0);
    }
    let mut written = 0u64;
    let mut skipped = 0u64;
    let mut offset: i64 = 0;
    loop {
        let rows = sqlx::query(
            "SELECT se.id, p.org_id AS workspace_id, se.project_id, se.kind, se.user_id, \
                    se.install_id, se.release, se.environment, se.country, se.asn, se.asn_org, \
                    se.server_name, se.data, se.occurred_at, se.received_at \
             FROM security_events se JOIN projects p ON p.id = se.project_id \
             ORDER BY se.received_at LIMIT $1 OFFSET $2",
        )
        .bind(PAGE)
        .bind(offset)
        .fetch_all(src)
        .await?;
        if rows.is_empty() {
            break;
        }
        report.note_read("security_events", rows.len() as u64);
        for r in &rows {
            if dry_run {
                continue;
            }
            let res = sqlx::query(
                "INSERT INTO security_events (id, workspace_id, project_id, kind, user_id, \
                    install_id, release, environment, country, asn, asn_org, server_name, \
                    data, occurred_at, received_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15) \
                 ON CONFLICT (id) DO NOTHING",
            )
            .bind(r.get::<uuid::Uuid, _>("id"))
            .bind(r.get::<uuid::Uuid, _>("workspace_id"))
            .bind(r.get::<uuid::Uuid, _>("project_id"))
            .bind(r.get::<String, _>("kind"))
            .bind(r.try_get::<Option<String>, _>("user_id").ok().flatten())
            .bind(r.try_get::<Option<String>, _>("install_id").ok().flatten())
            .bind(r.try_get::<Option<String>, _>("release").ok().flatten())
            .bind(r.try_get::<Option<String>, _>("environment").ok().flatten())
            .bind(r.try_get::<Option<String>, _>("country").ok().flatten())
            .bind(r.try_get::<Option<i32>, _>("asn").ok().flatten())
            .bind(r.try_get::<Option<String>, _>("asn_org").ok().flatten())
            .bind(r.try_get::<Option<String>, _>("server_name").ok().flatten())
            .bind(r.try_get::<Value, _>("data").unwrap_or(Value::Null))
            .bind(r.get::<time::OffsetDateTime, _>("occurred_at"))
            .bind(r.get::<time::OffsetDateTime, _>("received_at"))
            .execute(dst)
            .await?;
            if res.rows_affected() > 0 {
                written += 1;
            } else {
                skipped += 1;
            }
        }
        info!(offset, page = rows.len(), written, skipped, "security_events page");
        offset += PAGE;
        if rows.len() < PAGE as usize {
            break;
        }
    }
    report.note_written("security_events", written);
    report.note_skipped("security_events", skipped);
    Ok(written)
}

async fn federation(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    if !guard(src, "user_federation_links", report).await? {
        return Ok(0);
    }
    let rows = sqlx::query(
        "SELECT ufl.id, p.org_id AS workspace_id, ufl.project_id, ufl.provider, ufl.subject, \
                ufl.user_id, ufl.install_id, ufl.created_at \
         FROM user_federation_links ufl JOIN projects p ON p.id = ufl.project_id",
    )
    .fetch_all(src)
    .await?;
    report.note_read("user_federation_links", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO user_federation_links (id, workspace_id, project_id, provider, \
                subject, user_id, install_id, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8) ON CONFLICT (id) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("id"))
        .bind(r.get::<uuid::Uuid, _>("workspace_id"))
        .bind(r.get::<uuid::Uuid, _>("project_id"))
        .bind(r.get::<String, _>("provider"))
        .bind(r.get::<String, _>("subject"))
        .bind(r.try_get::<Option<String>, _>("user_id").ok().flatten())
        .bind(r.try_get::<Option<String>, _>("install_id").ok().flatten())
        .bind(r.get::<time::OffsetDateTime, _>("created_at"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("user_federation_links", written);
    report.note_skipped("user_federation_links", skipped);
    info!(read = rows.len(), written, skipped, "user_federation_links");
    Ok(written)
}

async fn user_reports(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    if !guard(src, "user_reports", report).await? {
        return Ok(0);
    }
    let rows = sqlx::query(
        "SELECT ur.id, p.org_id AS workspace_id, ur.project_id, ur.event_id, ur.issue_id, \
                ur.title, ur.body, ur.email, ur.name, ur.received_at \
         FROM user_reports ur JOIN projects p ON p.id = ur.project_id",
    )
    .fetch_all(src)
    .await?;
    report.note_read("user_reports", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO user_reports (id, workspace_id, project_id, event_id, issue_id, \
                title, body, email, name, received_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) ON CONFLICT (id) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("id"))
        .bind(r.get::<uuid::Uuid, _>("workspace_id"))
        .bind(r.get::<uuid::Uuid, _>("project_id"))
        .bind(r.try_get::<Option<uuid::Uuid>, _>("event_id").ok().flatten())
        .bind(r.try_get::<Option<uuid::Uuid>, _>("issue_id").ok().flatten())
        .bind(r.get::<String, _>("title"))
        .bind(r.get::<String, _>("body"))
        .bind(r.try_get::<Option<String>, _>("email").ok().flatten())
        .bind(r.try_get::<Option<String>, _>("name").ok().flatten())
        .bind(r.get::<time::OffsetDateTime, _>("received_at"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("user_reports", written);
    report.note_skipped("user_reports", skipped);
    info!(read = rows.len(), written, skipped, "user_reports");
    Ok(written)
}
