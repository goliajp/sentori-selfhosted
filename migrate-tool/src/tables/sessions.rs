//! sessions table (crash-free session pings).

use anyhow::Result;
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
    let mut written = 0u64;
    let mut skipped = 0u64;
    let mut offset: i64 = 0;
    loop {
        let rows = sqlx::query(
            "SELECT s.id, p.org_id AS workspace_id, s.project_id, s.user_id, s.release, \
                    s.environment, s.status, s.started_at, s.duration_ms, s.received_at \
             FROM sessions s JOIN projects p ON p.id = s.project_id \
             ORDER BY s.received_at LIMIT $1 OFFSET $2",
        )
        .bind(PAGE)
        .bind(offset)
        .fetch_all(src)
        .await?;
        if rows.is_empty() {
            break;
        }
        report.note_read("sessions", rows.len() as u64);
        for r in &rows {
            if dry_run {
                continue;
            }
            let res = sqlx::query(
                "INSERT INTO sessions (id, workspace_id, project_id, user_id, release, \
                    environment, status, started_at, duration_ms, received_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) ON CONFLICT (id) DO NOTHING",
            )
            .bind(r.get::<uuid::Uuid, _>("id"))
            .bind(r.get::<uuid::Uuid, _>("workspace_id"))
            .bind(r.get::<uuid::Uuid, _>("project_id"))
            .bind(r.try_get::<Option<String>, _>("user_id").ok().flatten())
            .bind(r.get::<String, _>("release"))
            .bind(r.get::<String, _>("environment"))
            .bind(r.get::<String, _>("status"))
            .bind(r.get::<time::OffsetDateTime, _>("started_at"))
            .bind(r.try_get::<i32, _>("duration_ms").unwrap_or(0))
            .bind(r.get::<time::OffsetDateTime, _>("received_at"))
            .execute(dst)
            .await?;
            if res.rows_affected() > 0 {
                written += 1;
            } else {
                skipped += 1;
            }
        }
        info!(offset, page = rows.len(), written, skipped, "sessions page");
        offset += PAGE;
        if rows.len() < PAGE as usize {
            break;
        }
    }
    report.note_written("sessions", written);
    report.note_skipped("sessions", skipped);
    Ok(written)
}
