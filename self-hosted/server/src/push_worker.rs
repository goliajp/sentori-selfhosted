//! Background push dispatcher worker.
//!
//! Drains `push_sends.status = 'queued'` every 5 seconds. For each
//! send, looks up the device_token + push_credentials, invokes the
//! configured vendor adapter (APNs / FCM / WebPush / HCM / MiPush),
//! and writes a `push_delivery_logs` row + flips
//! `push_sends.status` to `sent` or `failed`.
//!
//! v0.2 step 5 only ships a permissive "ack everything" mock
//! dispatcher because the vendor adapter crates (K7.1-K7.5) are
//! still being implemented. Production swaps in the real impls.
//!
//! Tunables (env-vars):
//! - `SENTORI_PUSH_WORKER_ENABLED`: 1/true to start the worker
//!   (default: enabled)
//! - `SENTORI_PUSH_WORKER_INTERVAL_SEC`: poll interval (default 5s)
//! - `SENTORI_PUSH_WORKER_BATCH`: max sends per poll (default 100)

use std::sync::Arc;
use std::time::Duration;

use sqlx::{PgPool, Row};
use tokio::time::sleep;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::token_cache::TokenCache;

/// Spawn the worker as a long-running tokio task.
pub fn spawn(pool: PgPool, cache: Arc<TokenCache>) {
    if !env_enabled() {
        info!("push worker disabled via SENTORI_PUSH_WORKER_ENABLED");
        return;
    }
    let interval = env_interval();
    let batch = env_batch();
    tokio::spawn(async move {
        info!(
            interval_sec = interval.as_secs(),
            batch, "push worker started"
        );
        loop {
            match drain_once(&pool, &cache, batch).await {
                Ok(0) => debug!("push worker idle"),
                Ok(n) => info!(processed = n, "push worker drained batch"),
                Err(e) => warn!(error = %e, "push worker batch failed"),
            }
            sleep(interval).await;
        }
    });
}

async fn drain_once(
    pool: &PgPool,
    cache: &Arc<TokenCache>,
    batch: usize,
) -> Result<usize, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, token_id, provider, payload FROM push_sends \
         WHERE status = 'queued' AND next_attempt_at <= now() \
         ORDER BY created_at LIMIT $1 FOR UPDATE SKIP LOCKED",
    )
    // Batch size is a small operator-set constant; saturating is
    // unreachable and a clamped LIMIT is harmless regardless.
    .bind(i64::try_from(batch).unwrap_or(i64::MAX))
    .fetch_all(pool)
    .await?;
    if rows.is_empty() {
        return Ok(0);
    }
    let mut processed = 0;
    for r in &rows {
        let send_id: Uuid = r.get("id");
        let provider: String = r.get("provider");
        if let Err(e) = dispatch_one(pool, cache, send_id, &provider).await {
            warn!(%send_id, error = %e, "push send dispatch failed");
            continue;
        }
        processed += 1;
    }
    Ok(processed)
}

const MAX_RETRIES: i32 = 5;

async fn dispatch_one(
    pool: &PgPool,
    cache: &Arc<TokenCache>,
    send_id: Uuid,
    provider: &str,
) -> Result<(), sqlx::Error> {
    use sqlx::Row;
    let send_meta = sqlx::query("SELECT token_id, retry_count FROM push_sends WHERE id = $1")
        .bind(send_id)
        .fetch_optional(pool)
        .await?;
    let (token_id, retry_count): (Option<Uuid>, i32) = match send_meta {
        Some(r) => (
            r.try_get("token_id").ok(),
            r.try_get("retry_count").unwrap_or(0),
        ),
        None => return Ok(()),
    };
    let attempt = retry_count + 1;

    let real_outcome = match provider {
        "webpush" => try_webpush(pool, send_id).await,
        "apns" => try_apns(pool, cache, send_id).await,
        "fcm" => try_fcm(pool, send_id).await,
        "hcm" => try_hcm(pool, cache, send_id).await,
        "mipush" => try_mipush(pool, send_id).await,
        _ => Err("provider_not_wired".to_string()),
    };

    match real_outcome {
        Ok((code, dur)) => {
            if let Some(t) = token_id {
                crate::push_quarantine::reset_streak(pool, t).await;
            }
            // Elapsed millis of a single HTTP send; saturates only
            // after ~24 days, far beyond the client timeout.
            let dur_ms = i32::try_from(dur).unwrap_or(i32::MAX);
            log_attempt(pool, send_id, attempt, "ok", i32::from(code), dur_ms).await?;
            mark_sent(pool, send_id, "ok").await?;
        }
        Err(reason) => {
            let http_status = extract_http_status(&reason).unwrap_or(0);
            let is_perm = http_status > 0
                && crate::push_quarantine::is_permanent_token_failure(
                    provider,
                    http_status,
                    &reason,
                );
            if let Some(t) = token_id {
                if is_perm {
                    crate::push_quarantine::quarantine_token(
                        pool,
                        t,
                        &format!("{provider}: HTTP {http_status}"),
                    )
                    .await;
                } else {
                    crate::push_quarantine::bump_streak(pool, t).await;
                }
            }
            log_attempt(
                pool,
                send_id,
                attempt,
                if is_perm {
                    "permanent_failure"
                } else {
                    "transient_failure"
                },
                i32::from(http_status),
                0,
            )
            .await?;
            if is_perm || is_configuration_failure(&reason) || retry_count >= MAX_RETRIES {
                mark_failed(pool, send_id, http_status, &reason).await?;
            } else {
                // Schedule retry with exponential backoff (30s, 60s,
                // 120s, 240s, 480s — cap 30min).
                let backoff = (30u64 * (1 << retry_count.min(6))).min(1800);
                requeue(pool, send_id, retry_count + 1, backoff, &reason).await?;
            }
        }
    }
    Ok(())
}

async fn log_attempt(
    pool: &PgPool,
    send_id: Uuid,
    attempt: i32,
    outcome: &str,
    provider_status: i32,
    duration_ms: i32,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO push_delivery_logs (id, send_id, attempt, outcome, provider_status, duration_ms) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(Uuid::now_v7())
    .bind(send_id)
    .bind(attempt)
    .bind(outcome)
    .bind(provider_status)
    .bind(duration_ms)
    .execute(pool)
    .await?;
    Ok(())
}

async fn mark_sent(pool: &PgPool, send_id: Uuid, outcome: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE push_sends SET status = 'sent', provider_outcome = $1, sent_at = now() \
         WHERE id = $2",
    )
    .bind(outcome)
    .bind(send_id)
    .execute(pool)
    .await?;
    Ok(())
}

async fn mark_failed(
    pool: &PgPool,
    send_id: Uuid,
    http_status: u16,
    reason: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE push_sends SET status = 'failed', provider_outcome = $1, error = $2 \
         WHERE id = $3",
    )
    .bind(format!("http_{http_status}"))
    .bind(reason.chars().take(500).collect::<String>())
    .bind(send_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// A failure that waiting cannot fix.
///
/// Retrying one of these five times over half an hour spends the
/// delay and arrives at the same place. Worse, insight watched a send
/// sit at `queued` with `error: null` for twenty minutes and had no
/// way to learn that the credential was the problem — a push that
/// could never go out is indistinguishable, from outside, from one
/// still on its way.
///
/// Matched on the text because that is what the adapters return.
/// Each of these is produced in exactly one place, and the test below
/// pins them to the strings those places emit.
fn is_configuration_failure(reason: &str) -> bool {
    reason == "credentials_missing"
        || reason == "provider_not_wired"
        // `FcmError::Credentials` — the JSON is not a service account.
        || reason.starts_with("service account json:")
        // `FcmError::OAuth` with a 4xx — Google refused the assertion,
        // which is the key or the clock, not the network.
        || reason.starts_with("oauth: status=4")
}

async fn requeue(
    pool: &PgPool,
    send_id: Uuid,
    new_retry_count: i32,
    backoff_sec: u64,
    reason: &str,
) -> Result<(), sqlx::Error> {
    // The reason travels with the retry. It used to be written only
    // when the send finally failed, so a receipt read during the
    // retries said `queued` and `error: null` — true, and useless.
    sqlx::query(
        "UPDATE push_sends SET status = 'queued', retry_count = $1, \
            next_attempt_at = now() + ($2 || ' seconds')::interval, \
            error = $4 \
         WHERE id = $3",
    )
    .bind(new_retry_count)
    .bind(backoff_sec.to_string())
    .bind(send_id)
    .bind(reason.chars().take(500).collect::<String>())
    .execute(pool)
    .await?;
    Ok(())
}

async fn try_hcm(
    pool: &PgPool,
    cache: &Arc<TokenCache>,
    send_id: Uuid,
) -> Result<(u16, u128), String> {
    use std::time::Instant;
    let row = sqlx::query(
        "SELECT dt.native_token, ps.payload, ps.project_id, pc.config, pc.secret_blob \
         FROM push_sends ps \
         JOIN device_tokens dt ON dt.id = ps.token_id \
         JOIN push_credentials pc ON pc.project_id = ps.project_id AND pc.kind = 'hcm' \
           AND pc.active \
         WHERE ps.id = $1",
    )
    .bind(send_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "credentials_missing".to_string())?;
    let project_id: Uuid = row.get("project_id");
    let device_token: String = row.get("native_token");
    let payload: serde_json::Value = row.get("payload");
    let config: serde_json::Value = row.get("config");
    let client_secret = String::from_utf8(row.get::<Vec<u8>, _>("secret_blob"))
        .map_err(|e| e.to_string())?
        .trim()
        .to_string();
    let client_id = config
        .get("clientId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "clientId missing".to_string())?
        .to_string();
    let app_id = config
        .get("appId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "appId missing".to_string())?
        .to_string();
    let title = payload
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Sentori");
    let body_text = payload.get("body").and_then(|v| v.as_str()).unwrap_or("");
    let cfg = crate::hcm::HcmConfig {
        client_id,
        client_secret,
        app_id,
    };

    let start = Instant::now();
    let token = if let Some(t) = cache.get(project_id, "hcm_oauth") {
        t
    } else {
        let t = crate::hcm::fetch_oauth_token(&cfg)
            .await
            .map_err(|e| e.to_string())?;
        cache.put(project_id, "hcm_oauth", t.clone(), Duration::from_mins(55));
        t
    };
    let status = crate::hcm::send_with_token(&cfg, &token, &device_token, title, body_text)
        .await
        .map_err(|e| e.to_string())?;
    Ok((status, start.elapsed().as_millis()))
}

async fn try_mipush(pool: &PgPool, send_id: Uuid) -> Result<(u16, u128), String> {
    use std::time::Instant;
    let row = sqlx::query(
        "SELECT dt.native_token, ps.payload, pc.config, pc.secret_blob \
         FROM push_sends ps \
         JOIN device_tokens dt ON dt.id = ps.token_id \
         JOIN push_credentials pc ON pc.project_id = ps.project_id AND pc.kind = 'mipush' \
           AND pc.active \
         WHERE ps.id = $1",
    )
    .bind(send_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "credentials_missing".to_string())?;
    let device_token: String = row.get("native_token");
    let payload: serde_json::Value = row.get("payload");
    let config: serde_json::Value = row.get("config");
    let app_secret = String::from_utf8(row.get::<Vec<u8>, _>("secret_blob"))
        .map_err(|e| e.to_string())?
        .trim()
        .to_string();
    let package_name = config
        .get("packageName")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "packageName missing".to_string())?
        .to_string();
    let title = payload
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Sentori");
    let body_text = payload.get("body").and_then(|v| v.as_str()).unwrap_or("");
    let cfg = crate::mipush::MiPushConfig {
        app_secret,
        package_name,
    };
    let start = Instant::now();
    let status = crate::mipush::send(&cfg, &device_token, title, body_text)
        .await
        .map_err(|e| e.to_string())?;
    Ok((status, start.elapsed().as_millis()))
}

async fn try_fcm(pool: &PgPool, send_id: Uuid) -> Result<(u16, u128), String> {
    use std::time::Instant;
    let row = sqlx::query(
        "SELECT dt.native_token, ps.payload, pc.secret_blob \
         FROM push_sends ps \
         JOIN device_tokens dt ON dt.id = ps.token_id \
         JOIN push_credentials pc ON pc.project_id = ps.project_id AND pc.kind = 'fcm' \
           AND pc.active \
         WHERE ps.id = $1",
    )
    .bind(send_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "credentials_missing".to_string())?;
    let device_token: String = row.get("native_token");
    let payload: serde_json::Value = row.get("payload");
    let service_account_json = String::from_utf8(row.get::<Vec<u8>, _>("secret_blob"))
        .map_err(|e| e.to_string())?
        .trim()
        .to_string();
    let title = payload
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Sentori");
    let body_text = payload.get("body").and_then(|v| v.as_str()).unwrap_or("");
    let cfg = crate::fcm::FcmConfig {
        service_account_json,
    };
    let start = Instant::now();
    let status = crate::fcm::send(&cfg, &device_token, title, body_text)
        .await
        .map_err(|e| e.to_string())?;
    Ok((status, start.elapsed().as_millis()))
}

/// Which of Apple's two hosts this send goes to.
///
/// The device knows: it registered with the env its build was signed
/// for, and a token minted against one host is refused by the other.
/// This read the *credential* instead, so a project with both a debug
/// build and a store build could only ever reach one of them — the
/// other half of the fleet came back `BadDeviceToken` and was
/// quarantined, each row keeping a reason that was never about it.
///
/// The credential's own setting is the fallback, for devices that
/// registered before the SDK sent an env.
fn apns_is_production(device_env: Option<&str>, config: &serde_json::Value) -> bool {
    match device_env {
        Some("sandbox") => false,
        Some("production") => true,
        _ => config
            .get("production")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true),
    }
}

async fn try_apns(
    pool: &PgPool,
    cache: &Arc<TokenCache>,
    send_id: Uuid,
) -> Result<(u16, u128), String> {
    use std::time::Instant;
    let row = sqlx::query(
        "SELECT dt.native_token, dt.env, ps.payload, ps.project_id, pc.config, pc.secret_blob \
         FROM push_sends ps \
         JOIN device_tokens dt ON dt.id = ps.token_id \
         JOIN push_credentials pc ON pc.project_id = ps.project_id AND pc.kind = 'apns' \
           AND pc.active \
         WHERE ps.id = $1",
    )
    .bind(send_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "credentials_missing".to_string())?;
    let project_id: Uuid = row.get("project_id");
    let device_token: String = row.get("native_token");
    let payload: serde_json::Value = row.get("payload");
    let config: serde_json::Value = row.get("config");
    let secret_pem =
        String::from_utf8(row.get::<Vec<u8>, _>("secret_blob")).map_err(|e| e.to_string())?;

    let team_id = config
        .get("teamId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "teamId missing".to_string())?
        .to_string();
    let key_id = config
        .get("keyId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "keyId missing".to_string())?
        .to_string();
    let topic = config
        .get("topic")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "topic missing".to_string())?
        .to_string();
    let production = apns_is_production(
        row.try_get::<Option<String>, _>("env")
            .ok()
            .flatten()
            .as_deref(),
        &config,
    );

    let title = payload
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Sentori");
    let body_text = payload.get("body").and_then(|v| v.as_str()).unwrap_or("");

    let cfg = crate::apns::ApnsConfig {
        team_id,
        key_id,
        topic,
        private_pem: secret_pem,
        production,
    };

    let start = Instant::now();
    // Try cached JWT first; on cache miss, send-with-mint inside
    // apns crate (which also returns the freshly-minted JWT for
    // us to cache going forward).
    let cached_jwt = cache.get(project_id, "apns_jwt");
    let status = if let Some(jwt) = cached_jwt {
        crate::apns::send_with_jwt(&cfg, &device_token, title, body_text, &jwt)
            .await
            .map_err(|e| e.to_string())?
    } else {
        let (status, jwt) = crate::apns::send_returning_jwt(&cfg, &device_token, title, body_text)
            .await
            .map_err(|e| e.to_string())?;
        cache.put(project_id, "apns_jwt", jwt, Duration::from_mins(55));
        status
    };
    Ok((status, start.elapsed().as_millis()))
}

async fn try_webpush(pool: &PgPool, send_id: Uuid) -> Result<(u16, u128), String> {
    use std::time::Instant;
    let row = sqlx::query(
        "SELECT dt.native_token, dt.metadata, ps.payload, pc.config, pc.secret_blob \
         FROM push_sends ps \
         JOIN device_tokens dt ON dt.id = ps.token_id \
         JOIN push_credentials pc ON pc.project_id = ps.project_id AND pc.kind = 'webpush' \
           AND pc.active \
         WHERE ps.id = $1",
    )
    .bind(send_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "credentials_missing".to_string())?;
    let endpoint: String = row.get("native_token");
    let metadata: serde_json::Value = row.try_get("metadata").unwrap_or(serde_json::Value::Null);
    let payload: serde_json::Value = row.try_get("payload").unwrap_or(serde_json::Value::Null);
    let config: serde_json::Value = row.get("config");
    let secret_pem =
        String::from_utf8(row.get::<Vec<u8>, _>("secret_blob")).map_err(|e| e.to_string())?;
    let subject = config
        .get("subject")
        .and_then(|v| v.as_str())
        .unwrap_or("mailto:admin@localhost")
        .to_string();
    let pub_key = config
        .get("vapidPublicKey")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let cfg = crate::webpush::WebPushConfig {
        vapid_subject: subject,
        vapid_public_key_b64url: pub_key,
        vapid_private_pem: secret_pem,
    };
    let start = Instant::now();
    // If the SDK persisted p256dh + auth_secret in
    // device_tokens.metadata, do an RFC 8291 encrypted send so the
    // browser actually displays the notification. Otherwise fall
    // back to wake-only push.
    let p256dh = metadata.get("p256dh").and_then(|v| v.as_str());
    let auth_secret = metadata.get("auth").and_then(|v| v.as_str());
    let status = if let (Some(p), Some(a)) = (p256dh, auth_secret) {
        let body_bytes = serde_json::to_vec(&payload).unwrap_or_default();
        crate::webpush::send_with_payload(&cfg, &endpoint, 3600, Some((&body_bytes, p, a)))
            .await
            .map_err(|e| e.to_string())?
    } else {
        crate::webpush::send(&cfg, &endpoint, 3600)
            .await
            .map_err(|e| e.to_string())?
    };
    let dur = start.elapsed().as_millis();
    Ok((status, dur))
}

#[allow(dead_code)]
fn mock_send(provider: &str) -> (&'static str, &'static str) {
    let _ = provider;
    ("sent", "ok")
}

/// Best-effort: parse an HTTP status code out of vendor error
/// strings like "apns rejected: status=410 body=...".
fn extract_http_status(s: &str) -> Option<u16> {
    let after = s.split("status=").nth(1)?;
    let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

fn env_enabled() -> bool {
    matches!(
        std::env::var("SENTORI_PUSH_WORKER_ENABLED")
            .ok()
            .as_deref()
            .map(str::to_ascii_lowercase),
        Some(s) if s == "1" || s == "true"
    ) || std::env::var("SENTORI_PUSH_WORKER_ENABLED").is_err()
}

fn env_interval() -> Duration {
    let secs = std::env::var("SENTORI_PUSH_WORKER_INTERVAL_SEC")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(5);
    Duration::from_secs(secs)
}

fn env_batch() -> usize {
    std::env::var("SENTORI_PUSH_WORKER_BATCH")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(100)
}

#[cfg(test)]
mod tests {
    /// The device's env decides the host, not the credential's.
    ///
    /// A project with a debug build and a store build registers both
    /// kinds. Reading the credential meant one of them was always sent
    /// to the wrong host, refused as `BadDeviceToken`, and quarantined
    /// for a reason that was about the host rather than the token.
    #[test]
    fn the_device_decides_which_apple_host_it_is_on() {
        let prod_cred = serde_json::json!({ "production": true });
        let sandbox_cred = serde_json::json!({ "production": false });

        assert!(!super::apns_is_production(Some("sandbox"), &prod_cred));
        assert!(super::apns_is_production(Some("production"), &sandbox_cred));

        // A device that registered before the SDK sent an env leaves
        // the choice where it was.
        assert!(super::apns_is_production(None, &prod_cred));
        assert!(!super::apns_is_production(None, &sandbox_cred));
        assert!(super::apns_is_production(None, &serde_json::json!({})));
        assert!(!super::apns_is_production(Some("odd"), &sandbox_cred));
    }

    use super::*;

    /// The classifier matches text, so the text has to be the text.
    ///
    /// Each string here is produced in exactly one place, and this
    /// pins the classifier to it: `FcmError`'s `#[error(...)]`
    /// attributes in `fcm.rs`, and the two literals this file returns
    /// when there is no credential row and no adapter.
    #[test]
    fn the_strings_it_matches_are_the_strings_that_are_produced() {
        let credentials = crate::fcm::FcmError::Credentials("expected value".into());
        assert!(
            is_configuration_failure(&credentials.to_string()),
            "a service-account paste that will not parse must fail at once, not \
             five times over half an hour: {credentials}"
        );

        let oauth = crate::fcm::FcmError::OAuth {
            status: 401,
            body: "invalid_grant".into(),
        };
        assert!(
            is_configuration_failure(&oauth.to_string()),
            "Google refusing the assertion is the key or the clock, not the \
             network: {oauth}"
        );

        assert!(is_configuration_failure("credentials_missing"));
        assert!(is_configuration_failure("provider_not_wired"));
    }

    /// And a real outage still retries. Classifying too much as
    /// permanent turns a blip into a lost notification.
    #[test]
    fn a_transient_failure_is_still_retried() {
        for reason in [
            "http: error sending request",
            "fcm rejected: status=503 body=backend unavailable",
            "oauth: status=500 body=internal",
            "fcm rejected: status=429 body=quota",
        ] {
            assert!(
                !is_configuration_failure(reason),
                "'{reason}' is worth retrying and this would have given up on it"
            );
        }
    }
}
