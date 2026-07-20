//! event_attachments + dsyms + proguard_mappings.
//!
//! Dst 0022 mirrors the legacy shapes verbatim and adds
//! workspace_id (derived via projects.org_id). All three are
//! empty in legacy prod — guarded, with real-shape queries so a
//! future nonzero run works.

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
    total += event_attachments(src, dst, dry_run, report).await?;
    total += dsyms(src, dst, dry_run, report).await?;
    total += proguard_mappings(src, dst, dry_run, report).await?;
    Ok(total)
}

async fn event_attachments(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    if !guard(src, "event_attachments", report).await? {
        return Ok(0);
    }
    // Legacy: ref (PK), event_id, project_id, kind, media_type,
    // size_bytes INT4, captured_at, source, received_at.
    let rows = sqlx::query(
        "SELECT a.ref, p.org_id AS workspace_id, a.project_id, a.event_id, a.kind, \
                a.media_type, a.size_bytes, a.captured_at, a.source, a.received_at \
         FROM event_attachments a JOIN projects p ON p.id = a.project_id",
    )
    .fetch_all(src)
    .await?;
    report.note_read("event_attachments", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO event_attachments (ref, workspace_id, project_id, event_id, kind, \
                media_type, size_bytes, captured_at, source, received_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) ON CONFLICT (ref) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("ref"))
        .bind(r.get::<uuid::Uuid, _>("workspace_id"))
        .bind(r.get::<uuid::Uuid, _>("project_id"))
        .bind(r.get::<uuid::Uuid, _>("event_id"))
        .bind(r.get::<String, _>("kind"))
        .bind(r.get::<String, _>("media_type"))
        .bind(r.get::<i32, _>("size_bytes"))
        .bind(r.get::<time::OffsetDateTime, _>("captured_at"))
        .bind(r.get::<String, _>("source"))
        .bind(r.get::<time::OffsetDateTime, _>("received_at"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("event_attachments", written);
    report.note_skipped("event_attachments", skipped);
    info!(read = rows.len(), written, skipped, "event_attachments");
    Ok(written)
}

async fn dsyms(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    if !guard(src, "dsyms", report).await? {
        return Ok(0);
    }
    // Legacy: id, project_id, release, debug_id, arch,
    // object_name, size_bytes INT4, data BYTEA (blob inline in
    // Postgres — legacy decision, mirrored by 0022), uploaded_by,
    // uploaded_at.
    let rows = sqlx::query(
        "SELECT d.id, p.org_id AS workspace_id, d.project_id, d.release, d.debug_id, \
                d.arch, d.object_name, d.size_bytes, d.data, d.uploaded_by, d.uploaded_at \
         FROM dsyms d JOIN projects p ON p.id = d.project_id",
    )
    .fetch_all(src)
    .await?;
    report.note_read("dsyms", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO dsyms (id, workspace_id, project_id, release, debug_id, arch, \
                object_name, size_bytes, data, uploaded_by, uploaded_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) ON CONFLICT (id) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("id"))
        .bind(r.get::<uuid::Uuid, _>("workspace_id"))
        .bind(r.get::<uuid::Uuid, _>("project_id"))
        .bind(r.get::<Option<String>, _>("release"))
        .bind(r.get::<String, _>("debug_id"))
        .bind(r.get::<String, _>("arch"))
        .bind(r.get::<Option<String>, _>("object_name"))
        .bind(r.get::<i32, _>("size_bytes"))
        .bind(r.get::<Vec<u8>, _>("data"))
        .bind(r.get::<Option<uuid::Uuid>, _>("uploaded_by"))
        .bind(r.get::<time::OffsetDateTime, _>("uploaded_at"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("dsyms", written);
    report.note_skipped("dsyms", skipped);
    info!(read = rows.len(), written, skipped, "dsyms");
    Ok(written)
}

async fn proguard_mappings(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    if !guard(src, "proguard_mappings", report).await? {
        return Ok(0);
    }
    // Legacy: id, project_id, release TEXT, debug_id, size_bytes
    // INT4, data BYTEA, uploaded_by, uploaded_at.
    let rows = sqlx::query(
        "SELECT pm.id, p.org_id AS workspace_id, pm.project_id, pm.release, pm.debug_id, \
                pm.size_bytes, pm.data, pm.uploaded_by, pm.uploaded_at \
         FROM proguard_mappings pm JOIN projects p ON p.id = pm.project_id",
    )
    .fetch_all(src)
    .await?;
    report.note_read("proguard_mappings", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO proguard_mappings (id, workspace_id, project_id, release, debug_id, \
                size_bytes, data, uploaded_by, uploaded_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) ON CONFLICT (id) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("id"))
        .bind(r.get::<uuid::Uuid, _>("workspace_id"))
        .bind(r.get::<uuid::Uuid, _>("project_id"))
        .bind(r.get::<Option<String>, _>("release"))
        .bind(r.get::<Option<String>, _>("debug_id"))
        .bind(r.get::<i32, _>("size_bytes"))
        .bind(r.get::<Vec<u8>, _>("data"))
        .bind(r.get::<Option<uuid::Uuid>, _>("uploaded_by"))
        .bind(r.get::<time::OffsetDateTime, _>("uploaded_at"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("proguard_mappings", written);
    report.note_skipped("proguard_mappings", skipped);
    info!(read = rows.len(), written, skipped, "proguard_mappings");
    Ok(written)
}
