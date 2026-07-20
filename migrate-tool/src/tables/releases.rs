//! releases + release_artifacts (deploy markers + symbolicator
//! blob metadata).

use anyhow::Result;
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
    total += releases(src, dst, dry_run, report).await?;
    total += release_artifacts(src, dst, dry_run, report).await?;
    Ok(total)
}

async fn releases(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    let rows = sqlx::query(
        "SELECT r.id, p.org_id, r.project_id, r.name, r.created_at, r.deploy_at \
         FROM releases r JOIN projects p ON p.id = r.project_id",
    )
    .fetch_all(src)
    .await?;
    report.note_read("releases", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO releases (id, workspace_id, project_id, name, created_at, deploy_at) \
             VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (id) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("id"))
        .bind(r.get::<uuid::Uuid, _>("org_id"))
        .bind(r.get::<uuid::Uuid, _>("project_id"))
        .bind(r.get::<String, _>("name"))
        .bind(r.get::<time::OffsetDateTime, _>("created_at"))
        .bind(r.try_get::<Option<time::OffsetDateTime>, _>("deploy_at").ok().flatten())
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("releases", written);
    report.note_skipped("releases", skipped);
    info!(read = rows.len(), written, skipped, "releases");
    Ok(written)
}

async fn release_artifacts(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    // Legacy release_artifacts: id, release_id, kind, name,
    // content_hash, blob_path, created_at, entry_count (INT4,
    // nullable), uncompressed_size_bytes (INT8, nullable),
    // module_label (nullable). Dst 0022/0017 mirrors the set and
    // adds workspace_id, derived via release → project → org.
    let rows = sqlx::query(
        "SELECT ra.id, p.org_id AS workspace_id, ra.release_id, ra.kind, ra.name, \
                ra.content_hash, ra.blob_path, ra.entry_count, \
                ra.uncompressed_size_bytes, ra.module_label, ra.created_at \
         FROM release_artifacts ra \
         JOIN releases r ON r.id = ra.release_id \
         JOIN projects p ON p.id = r.project_id",
    )
    .fetch_all(src)
    .await?;
    report.note_read("release_artifacts", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for row in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO release_artifacts (id, workspace_id, release_id, kind, name, \
                content_hash, blob_path, entry_count, uncompressed_size_bytes, \
                module_label, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) \
             ON CONFLICT (id) DO NOTHING",
        )
        .bind(row.get::<uuid::Uuid, _>("id"))
        .bind(row.get::<uuid::Uuid, _>("workspace_id"))
        .bind(row.get::<uuid::Uuid, _>("release_id"))
        .bind(row.get::<String, _>("kind"))
        .bind(row.get::<String, _>("name"))
        .bind(row.get::<String, _>("content_hash"))
        .bind(row.get::<String, _>("blob_path"))
        .bind(row.get::<Option<i32>, _>("entry_count"))
        .bind(row.get::<Option<i64>, _>("uncompressed_size_bytes"))
        .bind(row.get::<Option<String>, _>("module_label"))
        .bind(row.get::<time::OffsetDateTime, _>("created_at"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("release_artifacts", written);
    report.note_skipped("release_artifacts", skipped);
    info!(read = rows.len(), written, skipped, "release_artifacts");
    Ok(written)
}
