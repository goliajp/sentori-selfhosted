//! spans + traces (distributed tracing).

use anyhow::Result;
use serde_json::Value;
use sqlx::{PgPool, Row};
use tracing::info;

use crate::report::Report;

const PAGE: i64 = 2000;

pub async fn migrate(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    let mut total = 0u64;
    total += traces(src, dst, dry_run, report).await?;
    total += spans(src, dst, dry_run, report).await?;
    Ok(total)
}

async fn traces(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    let rows = sqlx::query(
        "SELECT t.trace_id, p.org_id AS workspace_id, t.project_id, t.root_op, t.root_name, \
                t.first_seen, t.last_seen, t.span_count, t.status, t.duration_ms \
         FROM traces t JOIN projects p ON p.id = t.project_id",
    )
    .fetch_all(src)
    .await?;
    report.note_read("traces", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO traces (trace_id, workspace_id, project_id, root_op, root_name, \
                first_seen, last_seen, span_count, status, duration_ms) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) \
             ON CONFLICT (trace_id) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("trace_id"))
        .bind(r.get::<uuid::Uuid, _>("workspace_id"))
        .bind(r.get::<uuid::Uuid, _>("project_id"))
        .bind(r.try_get::<Option<String>, _>("root_op").ok().flatten())
        .bind(r.try_get::<Option<String>, _>("root_name").ok().flatten())
        .bind(r.get::<time::OffsetDateTime, _>("first_seen"))
        .bind(r.get::<time::OffsetDateTime, _>("last_seen"))
        .bind(r.try_get::<i32, _>("span_count").unwrap_or(0))
        .bind(r.get::<String, _>("status"))
        .bind(r.try_get::<i32, _>("duration_ms").unwrap_or(0))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("traces", written);
    report.note_skipped("traces", skipped);
    info!(read = rows.len(), written, skipped, "traces");
    Ok(written)
}

async fn spans(
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
            "SELECT s.id, p.org_id AS workspace_id, s.project_id, s.trace_id, s.parent_span_id, \
                    s.received_at, s.started_at, s.duration_ms, s.op, s.name, s.status, \
                    s.tags, s.data, s.traceparent \
             FROM spans s JOIN projects p ON p.id = s.project_id \
             ORDER BY s.received_at LIMIT $1 OFFSET $2",
        )
        .bind(PAGE)
        .bind(offset)
        .fetch_all(src)
        .await?;
        if rows.is_empty() {
            break;
        }
        report.note_read("spans", rows.len() as u64);
        for r in &rows {
            if dry_run {
                continue;
            }
            let res = sqlx::query(
                "INSERT INTO spans (id, workspace_id, project_id, trace_id, parent_span_id, \
                    received_at, started_at, duration_ms, op, name, status, tags, data, traceparent) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14) \
                 ON CONFLICT (received_at, id) DO NOTHING",
            )
            .bind(r.get::<uuid::Uuid, _>("id"))
            .bind(r.get::<uuid::Uuid, _>("workspace_id"))
            .bind(r.get::<uuid::Uuid, _>("project_id"))
            .bind(r.get::<uuid::Uuid, _>("trace_id"))
            .bind(r.try_get::<Option<uuid::Uuid>, _>("parent_span_id").ok().flatten())
            .bind(r.get::<time::OffsetDateTime, _>("received_at"))
            .bind(r.get::<time::OffsetDateTime, _>("started_at"))
            .bind(r.try_get::<i32, _>("duration_ms").unwrap_or(0))
            .bind(r.get::<String, _>("op"))
            .bind(r.get::<String, _>("name"))
            .bind(r.get::<String, _>("status"))
            .bind(r.get::<Value, _>("tags"))
            .bind(r.try_get::<Option<Value>, _>("data").ok().flatten())
            .bind(r.try_get::<Option<String>, _>("traceparent").ok().flatten())
            .execute(dst)
            .await?;
            if res.rows_affected() > 0 {
                written += 1;
            } else {
                skipped += 1;
            }
        }
        info!(offset, page = rows.len(), written, skipped, "spans page");
        offset += PAGE;
        if rows.len() < PAGE as usize {
            break;
        }
    }
    report.note_written("spans", written);
    report.note_skipped("spans", skipped);
    Ok(written)
}
