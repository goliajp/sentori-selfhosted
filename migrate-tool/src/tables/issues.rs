//! issues table — INSERT-透传 with workspace_id rename.

use anyhow::Result;
use sqlx::{PgPool, Row};
use tracing::info;

use crate::report::Report;

const PAGE: i64 = 1000;

pub async fn migrate(
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
            "SELECT i.id, p.org_id AS workspace_id, i.project_id, i.fingerprint, i.error_type, \
                    i.message_sample, i.status, i.first_seen, i.last_seen, i.event_count, \
                    COALESCE(i.last_environment, '') AS last_environment, \
                    COALESCE(i.last_release, '') AS last_release, \
                    i.regressed_at, i.regressed_in_release, i.resolved_at \
             FROM issues i JOIN projects p ON p.id = i.project_id \
             ORDER BY i.last_seen LIMIT $1 OFFSET $2",
        )
        .bind(PAGE)
        .bind(offset)
        .fetch_all(src)
        .await?;
        if rows.is_empty() {
            break;
        }
        report.note_read("issues", rows.len() as u64);
        for r in &rows {
            if dry_run {
                continue;
            }
            let res = sqlx::query(
                "INSERT INTO issues (id, workspace_id, project_id, fingerprint, error_type, \
                    message_sample, kind, status, first_seen, last_seen, event_count, \
                    last_environment, last_release, regressed_at, regressed_in_release, resolved_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16) \
                 ON CONFLICT (id) DO NOTHING",
            )
            .bind(r.get::<uuid::Uuid, _>("id"))
            .bind(r.get::<uuid::Uuid, _>("workspace_id"))
            .bind(r.get::<uuid::Uuid, _>("project_id"))
            .bind(r.get::<String, _>("fingerprint"))
            .bind(r.get::<String, _>("error_type"))
            .bind(r.try_get::<String, _>("message_sample").unwrap_or_default())
            // Legacy has no kind column; every legacy issue is an
            // error-class issue.
            .bind("error")
            .bind(r.get::<String, _>("status"))
            .bind(r.get::<time::OffsetDateTime, _>("first_seen"))
            .bind(r.get::<time::OffsetDateTime, _>("last_seen"))
            .bind(r.get::<i64, _>("event_count"))
            .bind(r.get::<String, _>("last_environment"))
            .bind(r.get::<String, _>("last_release"))
            .bind(r.try_get::<Option<time::OffsetDateTime>, _>("regressed_at").ok().flatten())
            .bind(r.try_get::<Option<String>, _>("regressed_in_release").ok().flatten())
            .bind(r.try_get::<Option<time::OffsetDateTime>, _>("resolved_at").ok().flatten())
            .execute(dst)
            .await?;
            if res.rows_affected() > 0 {
                written += 1;
            } else {
                skipped += 1;
            }
        }
        info!(offset, page = rows.len(), written, skipped, "issues page");
        offset += PAGE;
        if rows.len() < PAGE as usize {
            break;
        }
    }
    report.note_written("issues", written);
    report.note_skipped("issues", skipped);
    Ok(written)
}
