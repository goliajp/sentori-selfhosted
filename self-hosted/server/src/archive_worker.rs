//! Background archive worker.
//!
//! Periodically (default daily) DELETEs three kinds of no-longer-
//! useful rows:
//!
//! - `sent` and old `failed` push_sends plus their delivery_logs.
//! - Rows with `expires_at < now()` in the four short-lived auth
//!   tables — sessions, email verifications, password resets, invites.
//!   The auth code already filters expired rows out of every lookup,
//!   so leaving them in the table is a correctness no-op but a slow
//!   size-forever leak that would eventually make those lookups
//!   scan through years of dead credentials.
//!
//! The `prune_expired` and `purge_*` store methods that do these
//! DELETEs have existed since the auth crate was written with no
//! caller. This is the caller.
//!
//! Since 2.2.0 it also owns the symbol-artifact retention story:
//!
//! - `release_artifacts` rows beyond the newest N releases per
//!   project are dropped (the release rows themselves stay — the
//!   regression anchor orders by `releases.created_at` and must
//!   keep seeing old releases). Symbol files age out of usefulness
//!   with the release; nobody symbolicates a two-year-old build.
//! - Orphaned blobs — hashes no `release_artifacts` or
//!   `event_attachments` row references — are deleted from the
//!   blob store. This is also what reclaims the old bytes after a
//!   re-upload replaces a row's content_hash in place.
//!
//! Tunables:
//! - `SENTORI_ARCHIVE_WORKER_ENABLED` default on
//! - `SENTORI_ARCHIVE_INTERVAL_SEC` default 86400 (24h)
//! - `SENTORI_ARCHIVE_SENT_DAYS`    default 30
//! - `SENTORI_ARCHIVE_FAILED_DAYS`  default 90
//! - `SENTORI_ARTIFACT_KEEP_RELEASES` default 20 (0 disables both
//!   the retention pass and the blob GC)
//! - `SENTORI_EVENT_RETENTION_DAYS` default 90 (0 disables) —
//!   events + their attachment rows past the window go; the issue
//!   rows they aggregated into stay (counters, first/last seen and
//!   the regression anchor are denormalized there). Attachment
//!   blobs become orphans and the GC pass reaps them.

use std::collections::HashSet;
use std::time::Duration;

use sqlx::PgPool;
use sqlx::Row;
use tokio::time::sleep;
use tracing::{info, warn};

use crate::blob_store::AttachmentStore;

pub fn spawn(pool: PgPool, attachments: AttachmentStore) {
    if !env_enabled() {
        info!("archive worker disabled via SENTORI_ARCHIVE_WORKER_ENABLED");
        return;
    }
    let interval = env_interval();
    let sent_days = env_sent_days();
    let failed_days = env_failed_days();
    let keep_releases = env_keep_releases();
    let event_days = env_event_retention_days();
    tokio::spawn(async move {
        info!(
            interval_sec = interval.as_secs(),
            sent_days, failed_days, keep_releases, event_days, "archive worker started"
        );
        loop {
            match run_once(&pool, sent_days, failed_days).await {
                Ok((sends, logs)) => info!(sends, logs, "archive worker pass"),
                Err(e) => warn!(error = %e, "archive worker pass failed"),
            }
            match prune_expired_auth(&pool).await {
                Ok((sessions, resets)) => {
                    info!(sessions, resets, "auth prune pass");
                }
                Err(e) => warn!(error = %e, "auth prune pass failed"),
            }
            if keep_releases > 0 {
                match prune_release_artifacts(&pool, keep_releases).await {
                    Ok(rows) => info!(rows, keep_releases, "artifact retention pass"),
                    Err(e) => warn!(error = %e, "artifact retention pass failed"),
                }
            }
            if event_days > 0 {
                match prune_old_events(&pool, event_days).await {
                    Ok((events, atts)) => {
                        info!(
                            events,
                            attachments = atts,
                            event_days,
                            "event retention pass"
                        );
                    }
                    Err(e) => warn!(error = %e, "event retention pass failed"),
                }
            }
            if keep_releases > 0 || event_days > 0 {
                match gc_orphan_blobs(&pool, &attachments).await {
                    Ok((deleted, kept)) => info!(deleted, kept, "orphan blob gc pass"),
                    Err(e) => warn!(error = %e, "orphan blob gc pass failed"),
                }
            }
            sleep(interval).await;
        }
    });
}

/// Drop symbol artifacts for everything older than the newest
/// `keep` releases of each project. Release rows survive — only
/// the artifact rows go; the blobs they pointed at become orphans
/// for [`gc_orphan_blobs`] to reap on the same pass.
async fn prune_release_artifacts(pool: &PgPool, keep: i64) -> Result<u64, sqlx::Error> {
    let rows = sqlx::query(
        "DELETE FROM release_artifacts WHERE release_id IN (             SELECT id FROM (                 SELECT id, row_number() OVER (                     PARTITION BY project_id ORDER BY created_at DESC                 ) AS rn FROM releases             ) ranked WHERE ranked.rn > $1          )",
    )
    .bind(keep)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(rows)
}

/// How long an unreferenced blob must have sat on disk before the
/// GC believes it is an orphan rather than an upload whose row has
/// not committed yet.
const ORPHAN_MIN_AGE: Duration = Duration::from_hours(1);

/// Delete blobs no DB row references. The mtime guard covers the
/// put-then-insert window of in-flight uploads.
async fn gc_orphan_blobs(
    pool: &PgPool,
    attachments: &AttachmentStore,
) -> Result<(u64, u64), Box<dyn std::error::Error + Send + Sync>> {
    let mut referenced: HashSet<String> = HashSet::new();
    for row in sqlx::query("SELECT content_hash FROM release_artifacts")
        .fetch_all(pool)
        .await?
    {
        referenced.insert(row.get::<String, _>(0));
    }
    for row in sqlx::query("SELECT blob_hash FROM event_attachments")
        .fetch_all(pool)
        .await?
    {
        referenced.insert(row.get::<String, _>(0));
    }

    let mut deleted = 0u64;
    let mut kept = 0u64;
    let now = std::time::SystemTime::now();
    for (hash, mtime) in attachments.list().await? {
        if referenced.contains(&hash.to_hex()) {
            kept += 1;
            continue;
        }
        let age = now.duration_since(mtime).unwrap_or(Duration::ZERO);
        if age < ORPHAN_MIN_AGE {
            kept += 1;
            continue;
        }
        // Re-check this one hash against the live table before
        // deleting it.
        //
        // The set above is a snapshot. Blobs are content-addressed, so
        // a blob whose last reference was deleted can be referenced
        // again by a fresh upload of the same bytes — `put` finds it
        // already on disk and returns without rewriting, leaving the
        // mtime from whenever it was first stored, often months back.
        // A reference that lands after the snapshot therefore clears
        // neither guard, and the GC deletes a blob a row now points
        // at: the user's upload succeeded and reads back 404.
        //
        // One query per deletion candidate, and candidates are by
        // definition the rare case.
        let hex = hash.to_hex();
        let still_orphan: Option<(i32,)> = sqlx::query_as(
            "SELECT 1 WHERE NOT EXISTS (SELECT 1 FROM event_attachments WHERE blob_hash = $1) \
                       AND NOT EXISTS (SELECT 1 FROM release_artifacts WHERE content_hash = $1)",
        )
        .bind(&hex)
        .fetch_optional(pool)
        .await?;
        if still_orphan.is_none() {
            kept += 1;
            continue;
        }

        match attachments.delete(&hash).await {
            Ok(()) => deleted += 1,
            Err(e) => warn!(hash = %hex, error = %e, "orphan blob delete failed"),
        }
    }
    Ok((deleted, kept))
}

/// Events (and their attachment rows) older than the window, in
/// batches so the first pass over a long backlog cannot hold a
/// giant delete transaction. `event_attachments.event_id` carries
/// no FK, so the rows go explicitly, attachments before events.
/// Issue aggregates are denormalized and survive; the attachment
/// blobs become orphans for [`gc_orphan_blobs`] on the same pass.
async fn prune_old_events(pool: &PgPool, days: i64) -> Result<(u64, u64), sqlx::Error> {
    const BATCH: i64 = 5_000;
    let mut events_total = 0u64;
    let mut atts_total = 0u64;
    loop {
        let ids: Vec<uuid::Uuid> = sqlx::query_scalar(
            "SELECT id FROM events              WHERE received_at < now() - ($1 || ' days')::interval              LIMIT $2",
        )
        .bind(days)
        .bind(BATCH)
        .fetch_all(pool)
        .await?;
        if ids.is_empty() {
            break;
        }
        atts_total += sqlx::query("DELETE FROM event_attachments WHERE event_id = ANY($1)")
            .bind(&ids)
            .execute(pool)
            .await?
            .rows_affected();
        events_total += sqlx::query("DELETE FROM events WHERE id = ANY($1)")
            .bind(&ids)
            .execute(pool)
            .await?
            .rows_affected();
    }
    Ok((events_total, atts_total))
}

fn env_event_retention_days() -> i64 {
    std::env::var("SENTORI_EVENT_RETENTION_DAYS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(90)
}

fn env_keep_releases() -> i64 {
    std::env::var("SENTORI_ARTIFACT_KEEP_RELEASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20)
}

async fn run_once(
    pool: &PgPool,
    sent_days: i32,
    failed_days: i32,
) -> Result<(u64, u64), sqlx::Error> {
    // Delete logs first (FK), then sends.
    let logs = sqlx::query(
        "DELETE FROM push_delivery_logs WHERE send_id IN ( \
            SELECT id FROM push_sends \
            WHERE (status = 'sent' AND created_at < now() - ($1 || ' days')::interval) \
               OR (status = 'failed' AND created_at < now() - ($2 || ' days')::interval) \
         )",
    )
    .bind(sent_days)
    .bind(failed_days)
    .execute(pool)
    .await?
    .rows_affected();

    let sends = sqlx::query(
        "DELETE FROM push_sends WHERE \
            (status = 'sent' AND created_at < now() - ($1 || ' days')::interval) \
            OR (status = 'failed' AND created_at < now() - ($2 || ' days')::interval)",
    )
    .bind(sent_days)
    .bind(failed_days)
    .execute(pool)
    .await?
    .rows_affected();

    Ok((sends, logs))
}

/// DELETE anything whose `expires_at` has already passed in the two
/// tables the auth flow relies on. Returns the row counts so the log
/// line names them; a run with zero everywhere is a healthy default.
async fn prune_expired_auth(pool: &PgPool) -> Result<(u64, u64), sqlx::Error> {
    let sessions = sqlx::query("DELETE FROM auth_sessions WHERE expires_at < now()")
        .execute(pool)
        .await?
        .rows_affected();
    let resets = sqlx::query("DELETE FROM password_resets WHERE expires_at < now()")
        .execute(pool)
        .await?
        .rows_affected();
    Ok((sessions, resets))
}

fn env_enabled() -> bool {
    matches!(
        std::env::var("SENTORI_ARCHIVE_WORKER_ENABLED")
            .ok()
            .as_deref()
            .map(str::to_ascii_lowercase),
        Some(s) if s == "1" || s == "true"
    ) || std::env::var("SENTORI_ARCHIVE_WORKER_ENABLED").is_err()
}

fn env_interval() -> Duration {
    let secs = std::env::var("SENTORI_ARCHIVE_INTERVAL_SEC")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(86400);
    Duration::from_secs(secs)
}

fn env_sent_days() -> i32 {
    std::env::var("SENTORI_ARCHIVE_SENT_DAYS")
        .ok()
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(30)
}

fn env_failed_days() -> i32 {
    std::env::var("SENTORI_ARCHIVE_FAILED_DAYS")
        .ok()
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(90)
}
