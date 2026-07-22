//! Catch-all for the remaining low-volume legacy tables:
//! - audit_object_versions (object diff history)
//! - cross_org_templates (legacy multi-org template library)
//! - label_catalog (workspace-wide label registry)
//! - pin_summaries (issue pin rollups)
//! - endpoint_probe_history (rollup, retained 30 days)
//!
//! Best-effort: each block is independent so missing source tables
//! don't break the others.

use anyhow::Result;
use serde_json::Value;
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
    total += audit_object_versions(src, dst, dry_run, report).await?;
    total += cross_org_templates(src, dst, dry_run, report).await?;
    total += label_catalog(src, dst, dry_run, report).await?;
    total += pin_summaries(src, dst, dry_run, report).await?;
    total += endpoint_probe_history(src, dst, dry_run, report).await?;
    Ok(total)
}

async fn audit_object_versions(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    let Ok(rows) = sqlx::query(
        "SELECT id, target_type, target_id, version, snapshot, created_at, created_by \
         FROM audit_object_versions",
    )
    .fetch_all(src)
    .await
    else {
        report.note_read("audit_object_versions", 0);
        return Ok(0);
    };
    report.note_read("audit_object_versions", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO audit_object_versions (id, target_type, target_id, version, snapshot, created_at, created_by) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT (id) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("id"))
        .bind(r.get::<String, _>("target_type"))
        .bind(r.get::<String, _>("target_id"))
        .bind(r.get::<i32, _>("version"))
        .bind(r.try_get::<Value, _>("snapshot").unwrap_or(Value::Null))
        .bind(r.get::<time::OffsetDateTime, _>("created_at"))
        .bind(r.try_get::<Option<uuid::Uuid>, _>("created_by").ok().flatten())
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("audit_object_versions", written);
    report.note_skipped("audit_object_versions", skipped);
    info!(read = rows.len(), written, skipped, "audit_object_versions");
    Ok(written)
}

async fn cross_org_templates(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    let Ok(rows) = sqlx::query(
        "SELECT id, kind, name, payload, created_at FROM cross_org_templates",
    )
    .fetch_all(src)
    .await
    else {
        report.note_read("cross_org_templates", 0);
        return Ok(0);
    };
    report.note_read("cross_org_templates", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO cross_org_templates (id, kind, name, payload, created_at) \
             VALUES ($1, $2, $3, $4, $5) ON CONFLICT (id) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("id"))
        .bind(r.get::<String, _>("kind"))
        .bind(r.get::<String, _>("name"))
        .bind(r.try_get::<Value, _>("payload").unwrap_or(Value::Null))
        .bind(r.get::<time::OffsetDateTime, _>("created_at"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("cross_org_templates", written);
    report.note_skipped("cross_org_templates", skipped);
    info!(read = rows.len(), written, skipped, "cross_org_templates");
    Ok(written)
}

async fn label_catalog(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    let Ok(rows) = sqlx::query(
        "SELECT lc.id, COALESCE(p.org_id, lc.workspace_id) AS workspace_id, lc.name, lc.color, lc.description, lc.created_at \
         FROM label_catalog lc LEFT JOIN projects p ON p.workspace_id = lc.workspace_id LIMIT 0",
    )
    .fetch_all(src)
    .await
    else {
        // Fall back to plain query without JOIN if workspace_id schema differs
        let Ok(rows) = sqlx::query(
            "SELECT id, workspace_id, name, color, description, created_at FROM label_catalog",
        )
        .fetch_all(src)
        .await
        else {
            report.note_read("label_catalog", 0);
            return Ok(0);
        };
        report.note_read("label_catalog", rows.len() as u64);
        let mut written = 0u64;
        let mut skipped = 0u64;
        for r in &rows {
            if dry_run {
                continue;
            }
            let res = sqlx::query(
                "INSERT INTO label_catalog (id, workspace_id, name, color, description, created_at) \
                 VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (id) DO NOTHING",
            )
            .bind(r.get::<uuid::Uuid, _>("id"))
            .bind(r.get::<uuid::Uuid, _>("workspace_id"))
            .bind(r.get::<String, _>("name"))
            .bind(r.try_get::<Option<String>, _>("color").ok().flatten())
            .bind(r.try_get::<Option<String>, _>("description").ok().flatten())
            .bind(r.get::<time::OffsetDateTime, _>("created_at"))
            .execute(dst)
            .await?;
            if res.rows_affected() > 0 {
                written += 1;
            } else {
                skipped += 1;
            }
        }
        report.note_written("label_catalog", written);
        report.note_skipped("label_catalog", skipped);
        info!(read = rows.len(), written, skipped, "label_catalog");
        return Ok(written);
    };
    let _ = rows;
    Ok(0)
}

async fn pin_summaries(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    let Ok(rows) = sqlx::query(
        "SELECT id, user_id, project_id, kind, target_id, summary, created_at \
         FROM pin_summaries",
    )
    .fetch_all(src)
    .await
    else {
        report.note_read("pin_summaries", 0);
        return Ok(0);
    };
    report.note_read("pin_summaries", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO pin_summaries (id, user_id, project_id, kind, target_id, summary, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT (id) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("id"))
        .bind(r.get::<uuid::Uuid, _>("user_id"))
        .bind(r.try_get::<Option<uuid::Uuid>, _>("project_id").ok().flatten())
        .bind(r.get::<String, _>("kind"))
        .bind(r.try_get::<Option<uuid::Uuid>, _>("target_id").ok().flatten())
        .bind(r.try_get::<Value, _>("summary").unwrap_or(Value::Null))
        .bind(r.get::<time::OffsetDateTime, _>("created_at"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("pin_summaries", written);
    report.note_skipped("pin_summaries", skipped);
    info!(read = rows.len(), written, skipped, "pin_summaries");
    Ok(written)
}

async fn endpoint_probe_history(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    let Ok(rows) = sqlx::query(
        "SELECT id, probe_id, status, observed_at, duration_ms \
         FROM endpoint_probe_history ORDER BY observed_at DESC LIMIT 100000",
    )
    .fetch_all(src)
    .await
    else {
        report.note_read("endpoint_probe_history", 0);
        return Ok(0);
    };
    report.note_read("endpoint_probe_history", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO endpoint_probe_history (id, probe_id, status, observed_at, duration_ms) \
             VALUES ($1, $2, $3, $4, $5) ON CONFLICT (id) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("id"))
        .bind(r.get::<uuid::Uuid, _>("probe_id"))
        .bind(r.get::<String, _>("status"))
        .bind(r.get::<time::OffsetDateTime, _>("observed_at"))
        .bind(r.try_get::<Option<i32>, _>("duration_ms").ok().flatten())
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("endpoint_probe_history", written);
    report.note_skipped("endpoint_probe_history", skipped);
    info!(read = rows.len(), written, skipped, "endpoint_probe_history");
    Ok(written)
}
