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
//! Tunables:
//! - `SENTORI_ARCHIVE_WORKER_ENABLED` default on
//! - `SENTORI_ARCHIVE_INTERVAL_SEC` default 86400 (24h)
//! - `SENTORI_ARCHIVE_SENT_DAYS`    default 30
//! - `SENTORI_ARCHIVE_FAILED_DAYS`  default 90

use std::time::Duration;

use sqlx::PgPool;
use tokio::time::sleep;
use tracing::{info, warn};

pub fn spawn(pool: PgPool) {
    if !env_enabled() {
        info!("archive worker disabled via SENTORI_ARCHIVE_WORKER_ENABLED");
        return;
    }
    let interval = env_interval();
    let sent_days = env_sent_days();
    let failed_days = env_failed_days();
    tokio::spawn(async move {
        info!(
            interval_sec = interval.as_secs(),
            sent_days, failed_days, "archive worker started"
        );
        loop {
            match run_once(&pool, sent_days, failed_days).await {
                Ok((sends, logs)) => info!(sends, logs, "archive worker pass"),
                Err(e) => warn!(error = %e, "archive worker pass failed"),
            }
            match prune_expired_auth(&pool).await {
                Ok((sessions, email, resets, invites)) => {
                    info!(sessions, email, resets, invites, "auth prune pass");
                }
                Err(e) => warn!(error = %e, "auth prune pass failed"),
            }
            sleep(interval).await;
        }
    });
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

/// DELETE anything whose `expires_at` has already passed in the four
/// tables the auth flow relies on. Returns the row counts so the log
/// line names them; a run with zero everywhere is a healthy default.
async fn prune_expired_auth(pool: &PgPool) -> Result<(u64, u64, u64, u64), sqlx::Error> {
    // Four small DELETEs on `expires_at < now()`. Each table is
    // separately indexed on expires_at at scales that matter, and a
    // single UNION would have to name them anyway.
    let sessions = sqlx::query("DELETE FROM auth_sessions WHERE expires_at < now()")
        .execute(pool)
        .await?
        .rows_affected();
    let email = sqlx::query("DELETE FROM email_verifications WHERE expires_at < now()")
        .execute(pool)
        .await?
        .rows_affected();
    let resets = sqlx::query("DELETE FROM password_resets WHERE expires_at < now()")
        .execute(pool)
        .await?
        .rows_affected();
    // Accepted invites carry `accepted_at` and are kept for audit; a
    // row with `expires_at < now()` AND no `accepted_at` is a bare
    // never-consumed invite and safe to drop.
    let invites = sqlx::query(
        "DELETE FROM workspace_invites WHERE expires_at < now() AND accepted_at IS NULL",
    )
    .execute(pool)
    .await?
    .rows_affected();
    Ok((sessions, email, resets, invites))
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
