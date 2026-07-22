//! Push platform tables: device_tokens, push_sends, device_topics,
//! push_preferences (dst 0024 mirrors legacy column names, plus
//! workspace_id derived via projects join).
//!
//! push_credentials is intentionally NOT migrated — see the
//! function comment (legacy-key-encrypted secrets).

use anyhow::Result;
use serde_json::Value;
use sqlx::{PgPool, Row};
use tracing::{info, warn};

use crate::report::Report;

use super::dashboard::guard;

pub async fn migrate(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    let mut total = 0u64;
    total += device_tokens(src, dst, dry_run, report).await?;
    total += push_credentials(src, dst, dry_run, report).await?;
    total += push_sends(src, dst, dry_run, report).await?;
    total += device_topics(src, dst, dry_run, report).await?;
    total += push_preferences(src, dst, dry_run, report).await?;
    Ok(total)
}

async fn device_tokens(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    let rows = sqlx::query(
        "SELECT dt.id, p.org_id AS workspace_id, dt.project_id, dt.provider, dt.env, \
                dt.native_token, dt.user_fingerprint_hex, dt.metadata, dt.bad_streak, \
                dt.revoked_at, dt.last_seen_at, dt.created_at, dt.updated_at \
         FROM device_tokens dt JOIN projects p ON p.id = dt.project_id",
    )
    .fetch_all(src)
    .await?;
    report.note_read("device_tokens", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO device_tokens (id, workspace_id, project_id, provider, env, \
                native_token, user_fingerprint_hex, metadata, bad_streak, revoked_at, \
                last_seen_at, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13) \
             ON CONFLICT (id) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("id"))
        .bind(r.get::<uuid::Uuid, _>("workspace_id"))
        .bind(r.get::<uuid::Uuid, _>("project_id"))
        .bind(r.get::<String, _>("provider"))
        .bind(r.try_get::<Option<String>, _>("env").ok().flatten())
        .bind(r.get::<String, _>("native_token"))
        .bind(r.try_get::<Option<Vec<u8>>, _>("user_fingerprint_hex").ok().flatten())
        .bind(r.try_get::<Value, _>("metadata").unwrap_or(Value::Null))
        .bind(r.try_get::<i32, _>("bad_streak").unwrap_or(0))
        .bind(r.try_get::<Option<time::OffsetDateTime>, _>("revoked_at").ok().flatten())
        .bind(r.get::<time::OffsetDateTime, _>("last_seen_at"))
        .bind(r.get::<time::OffsetDateTime, _>("created_at"))
        .bind(r.get::<time::OffsetDateTime, _>("updated_at"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("device_tokens", written);
    report.note_skipped("device_tokens", skipped);
    info!(read = rows.len(), written, skipped, "device_tokens");
    Ok(written)
}

async fn push_credentials(
    src: &PgPool,
    _dst: &PgPool,
    _dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    // Deliberately NOT migrated: legacy `secret_blob` is AES-GCM
    // ciphertext under the LEGACY server's master key — the v0.2
    // secrets vault cannot decrypt it, so copying the ciphertext
    // would be data corruption dressed as a migration.
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM push_credentials")
        .fetch_one(src)
        .await?;
    warn!(
        rows = n,
        "push_credentials: not migrated — secrets are encrypted under the \
         legacy master key; re-enter credentials in the dashboard"
    );
    report.note_read("push_credentials", n as u64);
    report.note_skipped("push_credentials", n as u64);
    Ok(0)
}

async fn push_sends(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    if !guard(src, "push_sends", report).await? {
        return Ok(0);
    }
    // dst 0024 mirrors legacy verbatim + workspace_id.
    let rows = sqlx::query(
        "SELECT s.id, p.org_id AS workspace_id, s.project_id, s.token_id, s.provider, \
                s.payload, s.status, s.provider_outcome, s.error, s.retry_count, \
                s.idempotency_key, s.next_attempt_at, s.created_at, s.sent_at, \
                s.campaign_id, s.template_id, s.audience_tag, s.acked_at, s.ack_session_id \
         FROM push_sends s JOIN projects p ON p.id = s.project_id",
    )
    .fetch_all(src)
    .await?;
    report.note_read("push_sends", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO push_sends (id, workspace_id, project_id, token_id, provider, \
                payload, status, provider_outcome, error, retry_count, idempotency_key, \
                next_attempt_at, created_at, sent_at, campaign_id, template_id, \
                audience_tag, acked_at, ack_session_id) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19) \
             ON CONFLICT (id) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("id"))
        .bind(r.get::<uuid::Uuid, _>("workspace_id"))
        .bind(r.get::<uuid::Uuid, _>("project_id"))
        .bind(r.get::<uuid::Uuid, _>("token_id"))
        .bind(r.get::<String, _>("provider"))
        .bind(r.get::<serde_json::Value, _>("payload"))
        .bind(r.get::<String, _>("status"))
        .bind(r.get::<Option<String>, _>("provider_outcome"))
        .bind(r.get::<Option<String>, _>("error"))
        .bind(r.get::<i32, _>("retry_count"))
        .bind(r.get::<Option<String>, _>("idempotency_key"))
        .bind(r.get::<time::OffsetDateTime, _>("next_attempt_at"))
        .bind(r.get::<time::OffsetDateTime, _>("created_at"))
        .bind(r.get::<Option<time::OffsetDateTime>, _>("sent_at"))
        .bind(r.get::<Option<String>, _>("campaign_id"))
        .bind(r.get::<Option<String>, _>("template_id"))
        .bind(r.get::<Option<String>, _>("audience_tag"))
        .bind(r.get::<Option<time::OffsetDateTime>, _>("acked_at"))
        .bind(r.get::<Option<String>, _>("ack_session_id"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("push_sends", written);
    report.note_skipped("push_sends", skipped);
    info!(read = rows.len(), written, skipped, "push_sends");
    Ok(written)
}

async fn device_topics(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    if !guard(src, "device_topics", report).await? {
        return Ok(0);
    }
    let rows =
        sqlx::query("SELECT device_token_id, topic, created_at FROM device_topics")
            .fetch_all(src)
            .await?;
    report.note_read("device_topics", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO device_topics (device_token_id, topic, created_at) \
             VALUES ($1, $2, $3) ON CONFLICT (device_token_id, topic) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("device_token_id"))
        .bind(r.get::<String, _>("topic"))
        .bind(r.get::<time::OffsetDateTime, _>("created_at"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("device_topics", written);
    report.note_skipped("device_topics", skipped);
    info!(read = rows.len(), written, skipped, "device_topics");
    Ok(written)
}

async fn push_preferences(
    src: &PgPool,
    dst: &PgPool,
    dry_run: bool,
    report: &mut Report,
) -> Result<u64> {
    if !guard(src, "push_preferences", report).await? {
        return Ok(0);
    }
    let rows = sqlx::query(
        "SELECT project_id, user_fingerprint_hex, category, opted_out, updated_at \
         FROM push_preferences",
    )
    .fetch_all(src)
    .await?;
    report.note_read("push_preferences", rows.len() as u64);
    let mut written = 0u64;
    let mut skipped = 0u64;
    for r in &rows {
        if dry_run {
            continue;
        }
        let res = sqlx::query(
            "INSERT INTO push_preferences (project_id, user_fingerprint_hex, category, \
                opted_out, updated_at) \
             VALUES ($1, $2, $3, $4, $5) ON CONFLICT (project_id, user_fingerprint_hex, category) DO NOTHING",
        )
        .bind(r.get::<uuid::Uuid, _>("project_id"))
        .bind(r.get::<Vec<u8>, _>("user_fingerprint_hex"))
        .bind(r.get::<String, _>("category"))
        .bind(r.get::<bool, _>("opted_out"))
        .bind(r.get::<time::OffsetDateTime, _>("updated_at"))
        .execute(dst)
        .await?;
        if res.rows_affected() > 0 {
            written += 1;
        } else {
            skipped += 1;
        }
    }
    report.note_written("push_preferences", written);
    report.note_skipped("push_preferences", skipped);
    info!(read = rows.len(), written, skipped, "push_preferences");
    Ok(written)
}
