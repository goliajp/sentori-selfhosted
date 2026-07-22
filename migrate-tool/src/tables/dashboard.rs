//! Dashboard / admin tables — saved_views, alert_rules,
//! integrations, audit_logs.
//!
//! Also hosts `src_count`, the shared guard used by every set
//! whose legacy table is empty or absent: to_regclass probe →
//! COUNT(*) → only then run the real query.

use anyhow::Result;
use serde_json::Value;
use sqlx::{PgPool, Row};
use tracing::{info, warn};

use crate::report::Report;

/// Guard probe for legacy tables that may be absent or empty.
///
/// Returns `None` if the table does not exist in the legacy DB,
/// otherwise `Some(row_count)`. Real query errors propagate.
pub(crate) async fn src_count(src: &PgPool, table: &str) -> Result<Option<i64>> {
    let exists: Option<String> = sqlx::query_scalar("SELECT to_regclass($1)::text")
        .bind(table)
        .fetch_one(src)
        .await?;
    if exists.is_none() {
        return Ok(None);
    }
    let n: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
        .fetch_one(src)
        .await?;
    Ok(Some(n))
}

/// Apply the uniform guard: `Ok(true)` means "proceed with the
/// real query"; `Ok(false)` means the caller should return 0
/// (absent → warn, empty → silent).
pub(crate) async fn guard(src: &PgPool, table: &str, report: &mut Report) -> Result<bool> {
    match src_count(src, table).await? {
        None => {
            warn!("{table}: not present in legacy, skipping");
            report.note_read(table, 0);
            Ok(false)
        }
        Some(0) => {
            report.note_read(table, 0);
            Ok(false)
        }
        Some(_) => Ok(true),
    }
}

pub async fn migrate(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    let mut total = 0u64;
    total += saved_views(src, dst, dry_run, report).await?;
    total += alert_rules(src, dst, dry_run, report).await?;
    total += integrations(src, dst, dry_run, report).await?;
    total += audit_logs(src, dst, dry_run, report).await?;
    Ok(total)
}

async fn saved_views(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    if !guard(src, "saved_views", report).await? {
        return Ok(0);
    }
    // Legacy: id, org_id, target, scope ('personal'|'team'|'org'),
    // team_id, user_id, name, payload, created_at, created_by,
    // updated_at. Dst 0014 has no team scope / team_id and no
    // org column (workspace_id instead); project_id is a new
    // nullable column → NULL for migrated org-level views.
    let rows = sqlx::query(
        "SELECT id, org_id, target, scope, team_id, user_id, name, payload, \
                created_at, created_by, updated_at \
         FROM saved_views",
    )
    .fetch_all(src)
    .await?;
    report.note_read("saved_views", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let scope: String = r.get("scope");
        // Dst 0014 only knows 'personal' | 'workspace'; legacy
        // 'org' maps to 'workspace', 'team' has no destination.
        let scope = match scope.as_str() {
            "org" => "workspace".to_string(),
            "team" => {
                skipped += 1;
                continue;
            }
            other => other.to_string(),
        };
        let res = sqlx::query(
            "INSERT INTO saved_views (id, workspace_id, project_id, target, scope, user_id, \
                name, payload, created_at, created_by, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) ON CONFLICT (id) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("id"))
        .bind(r.get::<uuid::Uuid, _>("org_id"))
        .bind(None::<uuid::Uuid>)
        .bind(r.get::<String, _>("target"))
        .bind(scope)
        .bind(r.get::<Option<uuid::Uuid>, _>("user_id"))
        .bind(r.get::<String, _>("name"))
        .bind(r.get::<Value, _>("payload"))
        .bind(r.get::<time::OffsetDateTime, _>("created_at"))
        .bind(r.get::<Option<uuid::Uuid>, _>("created_by"))
        .bind(r.get::<time::OffsetDateTime, _>("updated_at"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("saved_views", written);
    report.note_skipped("saved_views", skipped);
    info!(read = rows.len(), written, skipped, "saved_views");
    Ok(written)
}

async fn alert_rules(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    let rows = sqlx::query(
        "SELECT ar.id, COALESCE(p.org_id, '00000000-0000-0000-0000-000000000000'::uuid) AS workspace_id, \
                ar.project_id, ar.name, ar.enabled, ar.trigger_kind, ar.trigger_config, \
                ar.filter_config, ar.channels, ar.throttle_minutes, ar.last_fired_at, \
                ar.muted, ar.snoozed_until, ar.created_at, ar.created_by, ar.updated_at \
         FROM alert_rules ar LEFT JOIN projects p ON p.id = ar.project_id",
    )
    .fetch_all(src)
    .await?;
    report.note_read("alert_rules", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO alert_rules (id, workspace_id, project_id, name, enabled, \
                trigger_kind, trigger_config, filter_config, channels, throttle_minutes, \
                last_fired_at, muted, snoozed_until, created_at, created_by, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16) \
             ON CONFLICT (id) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("id"))
        .bind(r.get::<uuid::Uuid, _>("workspace_id"))
        .bind(r.try_get::<Option<uuid::Uuid>, _>("project_id").ok().flatten())
        .bind(r.get::<String, _>("name"))
        .bind(r.try_get::<bool, _>("enabled").unwrap_or(true))
        .bind(r.get::<String, _>("trigger_kind"))
        .bind(r.try_get::<Value, _>("trigger_config").unwrap_or(Value::Null))
        .bind(r.try_get::<Value, _>("filter_config").unwrap_or(Value::Null))
        .bind(r.try_get::<Value, _>("channels").unwrap_or(Value::Null))
        .bind(r.try_get::<i32, _>("throttle_minutes").unwrap_or(10))
        .bind(r.try_get::<Option<time::OffsetDateTime>, _>("last_fired_at").ok().flatten())
        .bind(r.try_get::<bool, _>("muted").unwrap_or(false))
        .bind(r.try_get::<Option<time::OffsetDateTime>, _>("snoozed_until").ok().flatten())
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
    report.note_written("alert_rules", written);
    report.note_skipped("alert_rules", skipped);
    info!(read = rows.len(), written, skipped, "alert_rules");
    Ok(written)
}

async fn integrations(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    if !guard(src, "integrations", report).await? {
        return Ok(0);
    }
    // Legacy: id, org_id, kind, config, created_at, revoked_at —
    // org-level rows with no project. Dst 0011 attaches per
    // project (project_id NOT NULL) → derive the org's oldest
    // project; orgs without a project skip the row. Mapping:
    // connected_at ← created_at, active ← revoked_at IS NULL,
    // connected_by ← NULL (legacy never recorded it).
    let rows = sqlx::query(
        "SELECT i.id, i.org_id, pj.id AS project_id, i.kind, i.config, \
                i.created_at, i.revoked_at \
         FROM integrations i \
         LEFT JOIN LATERAL ( \
             SELECT id FROM projects WHERE org_id = i.org_id \
             ORDER BY created_at LIMIT 1 \
         ) pj ON true",
    )
    .fetch_all(src)
    .await?;
    report.note_read("integrations", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let project_id: Option<uuid::Uuid> = r.get("project_id");
        let Some(project_id) = project_id else {
            skipped += 1;
            continue;
        };
        let revoked_at: Option<time::OffsetDateTime> = r.get("revoked_at");
        let res = sqlx::query(
            "INSERT INTO integrations (id, workspace_id, project_id, kind, config, \
                connected_by, connected_at, active) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8) ON CONFLICT (id) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("id"))
        .bind(r.get::<uuid::Uuid, _>("org_id"))
        .bind(project_id)
        .bind(r.get::<String, _>("kind"))
        .bind(r.get::<Value, _>("config"))
        .bind(None::<uuid::Uuid>)
        .bind(r.get::<time::OffsetDateTime, _>("created_at"))
        .bind(revoked_at.is_none())
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("integrations", written);
    report.note_skipped("integrations", skipped);
    info!(read = rows.len(), written, skipped, "integrations");
    Ok(written)
}

async fn audit_logs(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    // Legacy: id, org_id (nullable), actor_user_id, action,
    // target_type, target_id UUID, payload, created_at.
    // Dst: workspace_id NOT NULL, project_id nullable (new),
    // target_id TEXT → cast; rows without an org have no valid
    // workspace and are skipped.
    let rows = sqlx::query(
        "SELECT id, org_id, actor_user_id, action, target_type, \
                target_id::text AS target_id, payload, created_at \
         FROM audit_logs",
    )
    .fetch_all(src)
    .await?;
    report.note_read("audit_logs", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let org_id: Option<uuid::Uuid> = r.get("org_id");
        let Some(org_id) = org_id else {
            skipped += 1;
            continue;
        };
        let res = sqlx::query(
            "INSERT INTO audit_logs (id, workspace_id, project_id, actor_user_id, action, \
                target_type, target_id, payload, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) ON CONFLICT (id) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("id"))
        .bind(org_id)
        .bind(None::<uuid::Uuid>)
        .bind(r.get::<Option<uuid::Uuid>, _>("actor_user_id"))
        .bind(r.get::<String, _>("action"))
        .bind(r.get::<Option<String>, _>("target_type"))
        .bind(r.get::<Option<String>, _>("target_id"))
        .bind(r.get::<Value, _>("payload"))
        .bind(r.get::<time::OffsetDateTime, _>("created_at"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("audit_logs", written);
    report.note_skipped("audit_logs", skipped);
    info!(read = rows.len(), written, skipped, "audit_logs");
    Ok(written)
}
