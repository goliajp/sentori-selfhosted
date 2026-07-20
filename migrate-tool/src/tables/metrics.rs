//! Runtime-metric rollups — runtime_metrics_1m / _1h / _1d.
//!
//! Legacy and dst 0008 share the exact column set; dst adds
//! workspace_id NOT NULL (derived via projects.org_id).
//!
//! runtime_metrics_raw is intentionally NOT migrated: the dst
//! table is RANGE-partitioned by ts and partitions for the
//! historical legacy months do not exist — the 1m/1h/1d
//! aggregates carry the history instead.

use anyhow::Result;
use sqlx::{PgPool, Row};
use tracing::{info, warn};

use crate::report::Report;

pub async fn migrate(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    let mut total = 0u64;
    total += rollup(src, dst, dry_run, report, "runtime_metrics_1m").await?;
    total += rollup(src, dst, dry_run, report, "runtime_metrics_1h").await?;
    total += rollup(src, dst, dry_run, report, "runtime_metrics_1d").await?;
    warn!(
        "runtime_metrics_raw: intentionally not migrated — dst partitions for \
         historical months don't exist; 1m/1h/1d aggregates carry the history"
    );
    Ok(total)
}

async fn rollup(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
    table: &str,
) -> Result<u64> {
    // Legacy shape: bucket_ts, project_id, name, release,
    // environment, device_class (all three TEXT NOT NULL
    // DEFAULT ''), count, sum, avg, p50, p95, p99.
    let src_select = format!(
        "SELECT m.bucket_ts, p.org_id AS workspace_id, m.project_id, m.name, \
                m.release, m.environment, m.device_class, \
                m.count, m.sum, m.avg, m.p50, m.p95, m.p99 \
         FROM {table} m JOIN projects p ON p.id = m.project_id"
    );
    // Dst PK: (project_id, bucket_ts, name, release, environment, device_class).
    let dst_insert = format!(
        "INSERT INTO {table} (bucket_ts, workspace_id, project_id, name, release, \
            environment, device_class, count, sum, avg, p50, p95, p99) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13) \
         ON CONFLICT (project_id, bucket_ts, name, release, environment, device_class) \
         DO NOTHING"
    );
    let rows = sqlx::query(&src_select).fetch_all(src).await?;
    report.note_read(table, rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(&dst_insert)
            .bind(r.get::<time::OffsetDateTime, _>("bucket_ts"))
            .bind(r.get::<uuid::Uuid, _>("workspace_id"))
            .bind(r.get::<uuid::Uuid, _>("project_id"))
            .bind(r.get::<String, _>("name"))
            .bind(r.get::<String, _>("release"))
            .bind(r.get::<String, _>("environment"))
            .bind(r.get::<String, _>("device_class"))
            .bind(r.get::<i64, _>("count"))
            .bind(r.get::<f64, _>("sum"))
            .bind(r.get::<f64, _>("avg"))
            .bind(r.get::<f64, _>("p50"))
            .bind(r.get::<f64, _>("p95"))
            .bind(r.get::<f64, _>("p99"))
            .execute(dst)
            .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written(table, written);
    report.note_skipped(table, skipped);
    info!(read = rows.len(), written, skipped, table = %table, "runtime metric rollup");
    Ok(written)
}
