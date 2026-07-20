//! tokens table (SDK ingest tokens — `st_pk_<26 base32>`).

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
    let rows = sqlx::query(
        "SELECT id, org_id, project_id, kind, token_hash, label, last4, created_at, revoked_at \
         FROM tokens",
    )
    .fetch_all(src)
    .await?;
    report.note_read("tokens", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let id: uuid::Uuid = r.get("id");
        let workspace_id: uuid::Uuid = r.get("org_id");
        let project_id: uuid::Uuid = r.get("project_id");
        let kind: String = r.get("kind");
        // Legacy stores the SHA-256 hex-encoded as TEXT; dst 0016
        // mirrors that verbatim (TEXT, not BYTEA).
        let token_hash: String = r.get("token_hash");
        let label: Option<String> = r.try_get("label").ok();
        let last4: Option<String> = r.try_get("last4").ok();
        let created_at: time::OffsetDateTime = r.get("created_at");
        let revoked_at: Option<time::OffsetDateTime> = r.try_get("revoked_at").ok();
        let res = sqlx::query(
            "INSERT INTO tokens \
             (id, workspace_id, project_id, kind, token_hash, label, last4, created_at, revoked_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) ON CONFLICT (id) DO NOTHING",
        )
        .bind(id)
        .bind(workspace_id)
        .bind(project_id)
        .bind(&kind)
        .bind(&token_hash)
        .bind(label.as_deref())
        .bind(last4.as_deref())
        .bind(created_at)
        .bind(revoked_at)
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("tokens", written);
    report.note_skipped("tokens", skipped);
    info!(read = rows.len(), written, skipped, "tokens");
    Ok(written)
}
