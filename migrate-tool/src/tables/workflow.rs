//! Workflow engine tables — workflow_rules + workflow_runs +
//! workflow_run_steps. Legacy automation; v0.2 has no UI yet
//! but data is preserved.

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
    total += workflow_rules(src, dst, dry_run, report).await?;
    total += workflow_runs(src, dst, dry_run, report).await?;
    total += workflow_run_steps(src, dst, dry_run, report).await?;
    Ok(total)
}

async fn workflow_rules(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    if !guard(src, "workflow_rules", report).await? {
        return Ok(0);
    }
    let rows = sqlx::query(
        "SELECT wr.id, COALESCE(p.org_id, wr.workspace_id) AS workspace_id, wr.project_id, \
                wr.name, wr.trigger, wr.steps, wr.enabled, wr.created_at, wr.created_by, wr.updated_at \
         FROM workflow_rules wr LEFT JOIN projects p ON p.id = wr.project_id",
    )
    .fetch_all(src)
    .await?;
    report.note_read("workflow_rules", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO workflow_rules (id, workspace_id, project_id, name, trigger, steps, \
                enabled, created_at, created_by, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) ON CONFLICT (id) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("id"))
        .bind(r.get::<uuid::Uuid, _>("workspace_id"))
        .bind(r.try_get::<Option<uuid::Uuid>, _>("project_id").ok().flatten())
        .bind(r.get::<String, _>("name"))
        .bind(r.try_get::<Value, _>("trigger").unwrap_or(Value::Null))
        .bind(r.try_get::<Value, _>("steps").unwrap_or(Value::Null))
        .bind(r.try_get::<bool, _>("enabled").unwrap_or(true))
        .bind(r.get::<time::OffsetDateTime, _>("created_at"))
        .bind(r.try_get::<Option<uuid::Uuid>, _>("created_by").ok().flatten())
        .bind(r.get::<time::OffsetDateTime, _>("updated_at"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("workflow_rules", written);
    report.note_skipped("workflow_rules", skipped);
    info!(read = rows.len(), written, skipped, "workflow_rules");
    Ok(written)
}

async fn workflow_runs(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    if !guard(src, "workflow_runs", report).await? {
        return Ok(0);
    }
    let rows = sqlx::query(
        "SELECT id, rule_id, trigger_event_id, status, started_at, completed_at, error \
         FROM workflow_runs ORDER BY started_at DESC LIMIT 10000",
    )
    .fetch_all(src)
    .await?;
    report.note_read("workflow_runs", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO workflow_runs (id, rule_id, trigger_event_id, status, started_at, completed_at, error) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT (id) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("id"))
        .bind(r.get::<uuid::Uuid, _>("rule_id"))
        .bind(r.try_get::<Option<uuid::Uuid>, _>("trigger_event_id").ok().flatten())
        .bind(r.get::<String, _>("status"))
        .bind(r.get::<time::OffsetDateTime, _>("started_at"))
        .bind(r.try_get::<Option<time::OffsetDateTime>, _>("completed_at").ok().flatten())
        .bind(r.try_get::<Option<String>, _>("error").ok().flatten())
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("workflow_runs", written);
    report.note_skipped("workflow_runs", skipped);
    info!(read = rows.len(), written, skipped, "workflow_runs");
    Ok(written)
}

async fn workflow_run_steps(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    if !guard(src, "workflow_run_steps", report).await? {
        return Ok(0);
    }
    let rows = sqlx::query(
        "SELECT id, run_id, step_index, kind, status, output, started_at, completed_at, error \
         FROM workflow_run_steps ORDER BY started_at DESC LIMIT 50000",
    )
    .fetch_all(src)
    .await?;
    report.note_read("workflow_run_steps", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO workflow_run_steps (id, run_id, step_index, kind, status, output, \
                started_at, completed_at, error) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) ON CONFLICT (id) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("id"))
        .bind(r.get::<uuid::Uuid, _>("run_id"))
        .bind(r.get::<i32, _>("step_index"))
        .bind(r.get::<String, _>("kind"))
        .bind(r.get::<String, _>("status"))
        .bind(r.try_get::<Value, _>("output").unwrap_or(Value::Null))
        .bind(r.get::<time::OffsetDateTime, _>("started_at"))
        .bind(r.try_get::<Option<time::OffsetDateTime>, _>("completed_at").ok().flatten())
        .bind(r.try_get::<Option<String>, _>("error").ok().flatten())
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("workflow_run_steps", written);
    report.note_skipped("workflow_run_steps", skipped);
    info!(read = rows.len(), written, skipped, "workflow_run_steps");
    Ok(written)
}
