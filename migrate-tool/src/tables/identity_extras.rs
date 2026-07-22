//! Identity continuity tables — identity_scopes,
//! identity_fingerprints, identity_merges. Dst 0028 mirrors the
//! legacy shapes verbatim (no workspace_id on any of them).
//!
//! Note: identity.rs separately fills dst `privacy_salts` from
//! legacy identity_scopes — that is intentional and independent
//! of this passthrough.

use anyhow::Result;
use sqlx::{PgPool, Row};
use tracing::{info, warn};

use crate::report::Report;

use super::dashboard::src_count;

pub async fn migrate(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    let mut total = 0u64;
    total += identity_scopes(src, dst, dry_run, report).await?;
    total += identity_fingerprints(src, dst, dry_run, report).await?;
    total += identity_merges(src, dst, dry_run, report).await?;
    // Legacy has no saved_view_shares / pii_log tables (they were
    // an earlier assumption of this tool) — probe so the report
    // stays explicit about them being absent.
    for absent in ["saved_view_shares", "pii_log"] {
        match src_count(src, absent).await? {
            None => {
                warn!("{absent}: not present in legacy, skipping");
                report.note_read(absent, 0);
            }
            Some(0) => report.note_read(absent, 0),
            Some(n) => anyhow::bail!("{absent}: {n} legacy rows but no v0.2 destination table"),
        }
    }
    Ok(total)
}

async fn identity_scopes(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    // Legacy + dst 0028: id, name, salt BYTEA(32), created_at.
    let rows = sqlx::query("SELECT id, name, salt, created_at FROM identity_scopes")
        .fetch_all(src)
        .await?;
    report.note_read("identity_scopes", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO identity_scopes (id, name, salt, created_at) \
             VALUES ($1, $2, $3, $4) ON CONFLICT (id) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("id"))
        .bind(r.get::<String, _>("name"))
        .bind(r.get::<Vec<u8>, _>("salt"))
        .bind(r.get::<time::OffsetDateTime, _>("created_at"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("identity_scopes", written);
    report.note_skipped("identity_scopes", skipped);
    info!(read = rows.len(), written, skipped, "identity_scopes");
    Ok(written)
}

async fn identity_fingerprints(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    // Legacy + dst 0028: event_id, scope_id, key_type,
    // fingerprint BYTEA(32), received_at;
    // PK (event_id, scope_id, key_type).
    let rows = sqlx::query(
        "SELECT event_id, scope_id, key_type, fingerprint, received_at \
         FROM identity_fingerprints",
    )
    .fetch_all(src)
    .await?;
    report.note_read("identity_fingerprints", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO identity_fingerprints (event_id, scope_id, key_type, fingerprint, received_at) \
             VALUES ($1, $2, $3, $4, $5) \
             ON CONFLICT (event_id, scope_id, key_type) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("event_id"))
        .bind(r.get::<uuid::Uuid, _>("scope_id"))
        .bind(r.get::<String, _>("key_type"))
        .bind(r.get::<Vec<u8>, _>("fingerprint"))
        .bind(r.get::<time::OffsetDateTime, _>("received_at"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("identity_fingerprints", written);
    report.note_skipped("identity_fingerprints", skipped);
    info!(read = rows.len(), written, skipped, "identity_fingerprints");
    Ok(written)
}

async fn identity_merges(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    if !super::dashboard::guard(src, "identity_merges", report).await? {
        return Ok(0);
    }
    // Legacy + dst 0028: scope_id, primary_fp, alias_fp,
    // merged_by, merged_at, undone_at; PK (scope_id, alias_fp).
    let rows = sqlx::query(
        "SELECT scope_id, primary_fp, alias_fp, merged_by, merged_at, undone_at \
         FROM identity_merges",
    )
    .fetch_all(src)
    .await?;
    report.note_read("identity_merges", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO identity_merges (scope_id, primary_fp, alias_fp, merged_by, merged_at, undone_at) \
             VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (scope_id, alias_fp) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("scope_id"))
        .bind(r.get::<Vec<u8>, _>("primary_fp"))
        .bind(r.get::<Vec<u8>, _>("alias_fp"))
        .bind(r.get::<Option<uuid::Uuid>, _>("merged_by"))
        .bind(r.get::<time::OffsetDateTime, _>("merged_at"))
        .bind(r.get::<Option<time::OffsetDateTime>, _>("undone_at"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("identity_merges", written);
    report.note_skipped("identity_merges", skipped);
    info!(read = rows.len(), written, skipped, "identity_merges");
    Ok(written)
}
