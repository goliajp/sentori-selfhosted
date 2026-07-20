//! Identity layer ETL: orgs → workspaces, users, memberships,
//! projects, privacy_salts.
//!
//! Per inventory §5:
//! - legacy `orgs.id` → v0.2 `workspaces.id` (same UUID, table
//!   rename only)
//! - legacy `memberships.role` 4 levels (owner/admin/member/
//!   viewer) → v0.2 `workspace_members.role` 3 levels (viewer →
//!   user, member → user) — preserves read-only audit semantics
//!   via downstream ACL
//! - legacy `teams` / `team_memberships` / `project_teams` — NOT
//!   migrated in v0.2 (API hidden per §5.2; data preserved in
//!   legacy DB for future re-enable)
//!
//! Idempotent: ON CONFLICT (id) DO NOTHING throughout, so re-runs
//! after partial failure don't double-write.

use anyhow::Result;
use sqlx::{PgPool, Row};
use tracing::info;

use crate::report::Report;

pub async fn migrate_all(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    let mut total: u64 = 0;
    total += orgs_to_workspaces(src, dst, dry_run, report).await?;
    total += users(src, dst, dry_run, report).await?;
    total += memberships(src, dst, dry_run, report).await?;
    total += privacy_salts(src, dst, dry_run, report).await?;
    total += projects(src, dst, dry_run, report).await?;
    Ok(total)
}

async fn orgs_to_workspaces(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    let rows = sqlx::query("SELECT id, name, created_at FROM orgs")
        .fetch_all(src)
        .await?;
    report.note_read("orgs→workspaces", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let id: uuid::Uuid = r.get("id");
        let name: String = r.get("name");
        let created_at: time::OffsetDateTime = r.get("created_at");
        let res = sqlx::query(
            "INSERT INTO workspaces (id, name, created_at) VALUES ($1, $2, $3) \
             ON CONFLICT (id) DO NOTHING",
        )
        .bind(id)
        .bind(&name)
        .bind(created_at)
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("orgs→workspaces", written);
    report.note_skipped("orgs→workspaces", skipped);
    info!(read = rows.len(), written, skipped, "orgs→workspaces");
    Ok(written)
}

async fn users(src: &PgPool, dst: &PgPool, dry_run: bool, report: &mut Report) -> Result<u64> {
    // v0.2 users.workspace_id is NOT NULL. Derive from membership
    // (or, for users with no membership, fall back to the first
    // workspace as a salvage anchor).
    let rows = sqlx::query(
        "SELECT u.id, u.email, u.password_hash, u.email_verified, u.created_at, \
                COALESCE( \
                    (SELECT org_id FROM memberships m WHERE m.user_id = u.id ORDER BY created_at ASC LIMIT 1), \
                    (SELECT id FROM orgs ORDER BY created_at ASC LIMIT 1) \
                ) AS workspace_id \
         FROM users u",
    )
    .fetch_all(src)
    .await?;
    report.note_read("users", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let id: uuid::Uuid = r.get("id");
        let workspace_id: Option<uuid::Uuid> = r.try_get("workspace_id").ok();
        let Some(workspace_id) = workspace_id else {
            // No membership AND no workspace exists → can't assign.
            skipped += 1;
            continue;
        };
        let email: String = r.get("email");
        let password_hash: String = r.get("password_hash");
        let email_verified: bool = r.get("email_verified");
        let created_at: time::OffsetDateTime = r.get("created_at");
        let res = sqlx::query(
            "INSERT INTO users (id, workspace_id, email, password_hash, email_verified, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (id) DO NOTHING",
        )
        .bind(id)
        .bind(workspace_id)
        .bind(&email)
        .bind(&password_hash)
        .bind(email_verified)
        .bind(created_at)
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("users", written);
    report.note_skipped("users", skipped);
    info!(read = rows.len(), written, skipped, "users");
    Ok(written)
}

async fn memberships(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    // Legacy memberships carries only (org_id, user_id, role,
    // created_at) — no added_by/added_at; created_at maps to
    // dst added_at, added_by stays NULL.
    let rows = sqlx::query(
        "SELECT user_id, org_id, role, created_at FROM memberships",
    )
    .fetch_all(src)
    .await?;
    report.note_read("memberships", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let user_id: uuid::Uuid = r.get("user_id");
        let workspace_id: uuid::Uuid = r.get("org_id");
        let legacy_role: String = r.get("role");
        let role = map_role(&legacy_role);
        let added_by: Option<uuid::Uuid> = None;
        let added_at: time::OffsetDateTime = r.get("created_at");

        let res = sqlx::query(
            "INSERT INTO workspace_members (workspace_id, user_id, role, added_by, added_at) \
             VALUES ($1, $2, $3, $4, $5) ON CONFLICT DO NOTHING",
        )
        .bind(workspace_id)
        .bind(user_id)
        .bind(role)
        .bind(added_by)
        .bind(added_at)
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("memberships", written);
    report.note_skipped("memberships", skipped);
    info!(read = rows.len(), written, skipped, "memberships");
    Ok(written)
}

/// Slug for dst projects.slug (legacy projects has no slug column):
/// lowercase, runs of non-alphanumerics collapsed to '-'.
fn slugify(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut prev_dash = true;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() { "project".to_string() } else { out }
}

/// 4-level → 3-level role mapping.
///
/// - owner / admin → unchanged
/// - member → user (legacy "regular member with write access")
/// - viewer → user (legacy "read-only"; v0.2 enforces read-only
///   via separate ACL layer, role stays at "user" minimum tier)
fn map_role(legacy: &str) -> &'static str {
    match legacy {
        "owner" => "owner",
        "admin" => "admin",
        "member" | "viewer" => "user",
        _ => "user",
    }
}

async fn privacy_salts(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    // Legacy has no privacy_salts table — the 32-byte salts live in
    // identity_scopes (one per org, named '<org slug> (auto)').
    // Derive workspace_id from that naming convention; fall back to
    // the oldest org for scopes that don't match it.
    let rows = sqlx::query(
        "SELECT s.id, s.salt AS salt_bytes, s.created_at, \
                COALESCE( \
                    (SELECT o.id FROM orgs o WHERE s.name = o.slug || ' (auto)'), \
                    (SELECT id FROM orgs ORDER BY created_at ASC LIMIT 1) \
                ) AS org_id \
         FROM identity_scopes s",
    )
    .fetch_all(src)
    .await?;
    report.note_read("privacy_salts", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let id: uuid::Uuid = r.get("id");
        let workspace_id: Option<uuid::Uuid> = r.get("org_id");
        let Some(workspace_id) = workspace_id else {
            skipped += 1;
            continue;
        };
        let salt_bytes: Vec<u8> = r.get("salt_bytes");
        let created_at: time::OffsetDateTime = r.get("created_at");
        let res = sqlx::query(
            "INSERT INTO privacy_salts (id, workspace_id, salt_bytes, created_at) \
             VALUES ($1, $2, $3, $4) ON CONFLICT (id) DO NOTHING",
        )
        .bind(id)
        .bind(workspace_id)
        .bind(&salt_bytes)
        .bind(created_at)
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("privacy_salts", written);
    report.note_skipped("privacy_salts", skipped);
    info!(read = rows.len(), written, skipped, "privacy_salts");
    Ok(written)
}

async fn projects(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    // Legacy projects has no slug (synthesized from name below) and
    // its identity_scope_id is NULL in practice — resolve the salt
    // via the org's '<slug> (auto)' scope, falling back to the
    // oldest scope so dst's NOT NULL privacy_salt_id is satisfied.
    let rows = sqlx::query(
        "SELECT p.id, p.org_id, p.name, p.created_at, \
                COALESCE( \
                    p.identity_scope_id, \
                    (SELECT s.id FROM identity_scopes s \
                       JOIN orgs o ON s.name = o.slug || ' (auto)' \
                      WHERE o.id = p.org_id), \
                    (SELECT id FROM identity_scopes ORDER BY created_at ASC LIMIT 1) \
                ) AS privacy_salt_id \
         FROM projects p",
    )
    .fetch_all(src)
    .await?;
    report.note_read("projects", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let id: uuid::Uuid = r.get("id");
        let workspace_id: uuid::Uuid = r.get("org_id");
        let name: String = r.get("name");
        let slug: String = slugify(&name);
        let privacy_salt_id: Option<uuid::Uuid> = r.get("privacy_salt_id");
        let Some(privacy_salt_id) = privacy_salt_id else {
            // No scope exists at all — dst requires a salt; skip
            // rather than invent crypto material here.
            skipped += 1;
            continue;
        };
        let created_at: time::OffsetDateTime = r.get("created_at");
        let res = sqlx::query(
            "INSERT INTO projects (id, workspace_id, name, slug, privacy_salt_id, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (id) DO NOTHING",
        )
        .bind(id)
        .bind(workspace_id)
        .bind(&name)
        .bind(&slug)
        .bind(privacy_salt_id)
        .bind(created_at)
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("projects", written);
    report.note_skipped("projects", skipped);
    info!(read = rows.len(), written, skipped, "projects");
    Ok(written)
}
