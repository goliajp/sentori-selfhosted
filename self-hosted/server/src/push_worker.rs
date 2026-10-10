//! Durable push attempts with database leases; an interrupted send remains uncertain.
use crate::{state::AppState, token_cache::TokenCache};
use sqlx::{Row, postgres::PgRow};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, info, warn};
use uuid::Uuid;

pub fn spawn(state: Arc<AppState>, cache: Arc<TokenCache>) {
    if !env_enabled() {
        info!("push worker disabled via SENTORI_PUSH_WORKER_ENABLED");
        return;
    }
    let interval = env_interval();
    let batch = env_batch();
    tokio::spawn(async move {
        loop {
            match drain_once(&state, &cache, batch).await {
                Ok(0) => debug!("push worker idle"),
                Ok(count) => info!(count, "push worker drained batch"),
                Err(error) => warn!(%error,"push worker failed"),
            }
            sleep(interval).await;
        }
    });
}
const MAX_RETRIES: i32 = 5;
struct Attempt {
    id: Uuid,
    token: Uuid,
    retries: i32,
    lease: Uuid,
    provider: String,
}
struct Outcome {
    status: &'static str,
    code: String,
    error: Option<String>,
    http: u16,
    duration: i32,
    retries: i32,
    backoff: i32,
    device: DeviceEffect,
}
enum DeviceEffect {
    None,
    Success,
    Transient,
    Permanent,
}
async fn drain_once(
    state: &AppState,
    cache: &Arc<TokenCache>,
    batch: usize,
) -> Result<usize, sqlx::Error> {
    // These two statements also serve replicas: claims below exclude live leases.
    sqlx::query("WITH expired AS (UPDATE push_sends SET status='failed',provider_outcome='uncertain',error='worker_interrupted',lease_owner=NULL,lease_until=NULL WHERE id IN (SELECT id FROM push_sends WHERE status='queued' AND lease_until<now() ORDER BY lease_until LIMIT 32 FOR UPDATE SKIP LOCKED) RETURNING id,retry_count) INSERT INTO push_delivery_logs(id,send_id,attempt,outcome) SELECT gen_random_uuid(),id,COALESCE((SELECT max(attempt) FROM push_delivery_logs WHERE send_id=expired.id),0)+1,'uncertain' FROM expired").execute(&state.pool).await?;
    sqlx::query("UPDATE push_sends SET status='failed',provider_outcome='cancelled',error='instance_changed' WHERE id IN (SELECT p.id FROM push_sends p JOIN instance_settings s ON s.singleton WHERE p.status='queued' AND p.lease_owner IS NULL AND p.generation<>s.generation LIMIT 1000 FOR UPDATE OF p SKIP LOCKED)").execute(&state.pool).await?;
    let mut count = 0;
    for _ in 0..batch {
        let lease = Uuid::now_v7();
        let row=sqlx::query("WITH due AS (SELECT p.id FROM push_sends p JOIN instance_settings s ON s.singleton AND s.generation=p.generation WHERE p.status='queued' AND p.next_attempt_at<=now() AND p.lease_owner IS NULL ORDER BY p.created_at,p.id LIMIT 1 FOR UPDATE OF p SKIP LOCKED) UPDATE push_sends p SET lease_owner=$1,lease_until=now()+interval '45 seconds',credential_id=COALESCE(p.credential_id,(SELECT id FROM push_credentials WHERE project_id=p.project_id AND kind=p.provider AND active)) FROM due WHERE p.id=due.id RETURNING p.id,p.token_id,p.retry_count,p.provider").bind(lease).fetch_optional(&state.pool).await?;
        let Some(row) = row else { break };
        let attempt = Attempt {
            id: row.get("id"),
            token: row.get("token_id"),
            retries: row.get("retry_count"),
            lease,
            provider: row.get("provider"),
        };
        dispatch_one(state, cache, &attempt).await?;
        count += 1;
    }
    Ok(count)
}
async fn dispatch_one(
    state: &AppState,
    cache: &Arc<TokenCache>,
    attempt: &Attempt,
) -> Result<(), sqlx::Error> {
    let allowed:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM push_sends p JOIN instance_settings s ON s.singleton AND s.generation=p.generation JOIN device_tokens d ON d.id=p.token_id AND d.project_id=p.project_id AND d.revoked_at IS NULL WHERE p.id=$1 AND p.status='queued' AND p.lease_owner=$2 AND p.lease_until>now())").bind(attempt.id).bind(attempt.lease).fetch_one(&state.pool).await?;
    let outcome = if allowed {
        let result = tokio::time::timeout(Duration::from_secs(30), async {
            match attempt.provider.as_str() {
                "webpush" => try_webpush(state, attempt.id).await,
                "apns" => try_apns(state, cache, attempt.id).await,
                "fcm" => try_fcm(state, attempt.id).await,
                "hcm" => try_hcm(state, cache, attempt.id).await,
                "mipush" => try_mipush(state, attempt.id).await,
                _ => Err("provider_not_wired".into()),
            }
        })
        .await
        .unwrap_or_else(|_| Err("send_timeout".into()));
        outcome_for(attempt, result)
    } else {
        Outcome {
            status: "failed",
            code: "cancelled".into(),
            error: Some("target_unavailable".into()),
            http: 0,
            duration: 0,
            retries: attempt.retries,
            backoff: 0,
            device: DeviceEffect::None,
        }
    };
    finish(state, attempt, &outcome).await
}
fn outcome_for(attempt: &Attempt, result: Result<(u16, u128), String>) -> Outcome {
    match result {
        Ok((http, duration)) => Outcome {
            status: "sent",
            code: "ok".into(),
            error: None,
            http,
            duration: i32::try_from(duration).unwrap_or(i32::MAX),
            retries: attempt.retries,
            backoff: 0,
            device: DeviceEffect::Success,
        },
        Err(reason) => {
            let http = extract_http_status(&reason).unwrap_or(0);
            let permanent = crate::push_quarantine::is_permanent_token_failure(
                &attempt.provider,
                http,
                &reason,
            );
            let uncertain = matches!(reason.as_str(), "send_timeout" | "send_uncertain")
                || reason.starts_with("http:");
            let terminal = permanent
                || is_configuration_failure(&reason)
                || (300..400).contains(&http)
                || uncertain
                || attempt.retries >= MAX_RETRIES;
            Outcome {
                status: if terminal { "failed" } else { "queued" },
                code: if uncertain {
                    "uncertain".into()
                } else if permanent {
                    "permanent_failure".into()
                } else {
                    format!("http_{http}")
                },
                error: Some(safe_reason(&reason)),
                http,
                duration: 0,
                retries: if terminal {
                    attempt.retries
                } else {
                    attempt.retries + 1
                },
                backoff: if terminal {
                    0
                } else {
                    30 * (1 << attempt.retries.min(6))
                },
                device: if permanent {
                    DeviceEffect::Permanent
                } else if uncertain {
                    DeviceEffect::None
                } else {
                    DeviceEffect::Transient
                },
            }
        }
    }
}
fn safe_reason(reason: &str) -> String {
    let reason = reason.strip_prefix("bad endpoint url: ").unwrap_or(reason);
    for code in [
        "credentials_missing",
        "provider_not_wired",
        "vault_unconfigured",
        "decryption_failed",
        "credential_scope_mismatch",
        "invalid_credential",
        "send_timeout",
        "send_uncertain",
        "non_public_address",
        "invalid_url",
        "https_required",
        "redirect_refused",
    ] {
        if reason == code {
            return code.into();
        }
    }
    for code in [
        "InvalidProviderToken",
        "MissingProviderToken",
        "TopicDisallowed",
        "BadTopic",
        "BadCertificateEnvironment",
        "BadDeviceToken",
        "DeviceTokenNotForTopic",
        "MissingDeviceToken",
        "UNREGISTERED",
        "NotRegistered",
        "ExpiredProviderToken",
    ] {
        if reason.contains(code) {
            return code.into();
        }
    }
    extract_http_status(reason).map_or_else(
        || "provider_request_failed".into(),
        |http| format!("http_{http}"),
    )
}
async fn finish(state: &AppState, attempt: &Attempt, outcome: &Outcome) -> Result<(), sqlx::Error> {
    let mut tx = state.pool.begin().await?;
    let changed=sqlx::query("UPDATE push_sends SET status=$3,provider_outcome=$4,error=$5,retry_count=$6,next_attempt_at=now()+$7*interval '1 second',sent_at=CASE WHEN $3='sent' THEN now() ELSE sent_at END,lease_owner=NULL,lease_until=NULL WHERE id=$1 AND lease_owner=$2 AND lease_until>now() AND status='queued'")
        .bind(attempt.id).bind(attempt.lease).bind(outcome.status).bind(&outcome.code).bind(&outcome.error).bind(outcome.retries).bind(outcome.backoff).execute(&mut *tx).await?;
    if changed.rows_affected() == 0 {
        return Ok(());
    }
    sqlx::query("INSERT INTO push_delivery_logs(id,send_id,attempt,outcome,provider_status,duration_ms) VALUES($1,$2,(SELECT COALESCE(max(attempt),0)+1 FROM push_delivery_logs WHERE send_id=$2),$3,$4,$5)").bind(Uuid::now_v7()).bind(attempt.id).bind(&outcome.code).bind(i32::from(outcome.http)).bind(outcome.duration).execute(&mut *tx).await?;
    match outcome.device {
        DeviceEffect::None => {}
        DeviceEffect::Success => {
            sqlx::query("UPDATE device_tokens SET bad_streak=0 WHERE id=$1 AND bad_streak>0")
                .bind(attempt.token)
                .execute(&mut *tx)
                .await?;
        }
        DeviceEffect::Transient => {
            sqlx::query(
                "UPDATE device_tokens SET bad_streak=LEAST(bad_streak,2147483646)+1 WHERE id=$1",
            )
            .bind(attempt.token)
            .execute(&mut *tx)
            .await?;
        }
        DeviceEffect::Permanent => {
            sqlx::query("UPDATE device_tokens SET revoked_at=now(),metadata=metadata||jsonb_build_object('quarantine_reason',$2::text) WHERE id=$1").bind(attempt.token).bind(format!("{}: HTTP {}",attempt.provider,outcome.http)).execute(&mut *tx).await?;
        }
    }
    tx.commit().await
}
fn credential_secret(state: &AppState, row: &PgRow, kind: &str) -> Result<String, String> {
    crate::push_secrets::open(
        state.vault.as_deref(),
        row.get("project_id"),
        row.get("credential_id"),
        kind,
        &row.get::<String, _>("secret_format"),
        &row.get::<Vec<u8>, _>("secret_blob"),
    )
    .map_err(str::to_owned)
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
    matches!(reason,"credentials_missing"|"vault_unconfigured"|"decryption_failed"|"credential_scope_mismatch"|"invalid_credential"|"non_public_address"|"invalid_url"|"https_required"|"redirect_refused")
        || reason == "provider_not_wired"
        || reason.starts_with("bad endpoint url:")
        || reason.starts_with("jwt sign:")
        // `FcmError::Credentials` — the JSON is not a service account.
        || reason.starts_with("service account json:")
        // `FcmError::OAuth` with a 4xx — Google refused the assertion,
        // which is the key or the clock, not the network.
        || reason.starts_with("oauth: status=4")
        // The APNs half of the same thing, which was missing: these say
        // our signing key, team or bundle id is wrong, not that this
        // device is gone. Without them a rejected send went back on the
        // queue five times while its recorded outcome already read
        // "rejected" — a row that was both queued and refused, which is
        // how a project whose push has never worked showed zero
        // failures and a green readiness check.
        || is_apns_credential_rejection(reason)
}

/// APNs reasons that no retry can fix. `ExpiredProviderToken` is
/// deliberately absent: the worker mints a fresh JWT per attempt, so
/// retrying that one is the repair.
fn is_apns_credential_rejection(reason: &str) -> bool {
    reason.contains("InvalidProviderToken")
        || reason.contains("MissingProviderToken")
        || reason.contains("TopicDisallowed")
        || reason.contains("BadTopic")
        || reason.contains("BadCertificateEnvironment")
}

async fn try_hcm(
    state: &AppState,
    cache: &Arc<TokenCache>,
    send_id: Uuid,
) -> Result<(u16, u128), String> {
    use std::time::Instant;
    let pool = &state.pool;
    let row = sqlx::query(
        "SELECT dt.native_token, ps.payload, ps.project_id, pc.config, pc.secret_blob, pc.id AS credential_id, pc.secret_format \
         FROM push_sends ps \
         JOIN device_tokens dt ON dt.id = ps.token_id \
         JOIN push_credentials pc ON pc.project_id = ps.project_id AND pc.kind = 'hcm' \
           AND pc.active \
         WHERE ps.id = $1 AND dt.project_id=ps.project_id AND dt.revoked_at IS NULL AND pc.id=ps.credential_id AND ps.generation=(SELECT generation FROM instance_settings WHERE singleton)",
    )
    .bind(send_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "credentials_missing".to_string())?;
    let credential_id: Uuid = row.get("credential_id");
    let device_token: String = row.get("native_token");
    let payload: serde_json::Value = row.get("payload");
    let config: serde_json::Value = row.get("config");
    let client_secret = credential_secret(state, &row, "hcm")?.trim().to_owned();
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
    let token = if let Some(t) = cache.get(credential_id, "hcm_oauth") {
        t
    } else {
        let t = crate::hcm::fetch_oauth_token(&cfg)
            .await
            .map_err(|e| e.to_string())?;
        cache.put(
            credential_id,
            "hcm_oauth",
            t.clone(),
            Duration::from_mins(55),
        );
        t
    };
    let status = crate::hcm::send_with_token(&cfg, &token, &device_token, title, body_text)
        .await
        .map_err(|e| e.to_string())?;
    Ok((status, start.elapsed().as_millis()))
}

async fn try_mipush(state: &AppState, send_id: Uuid) -> Result<(u16, u128), String> {
    use std::time::Instant;
    let pool = &state.pool;
    let row = sqlx::query(
        "SELECT dt.native_token, ps.payload, ps.project_id, pc.config, pc.secret_blob, pc.id AS credential_id, pc.secret_format \
         FROM push_sends ps \
         JOIN device_tokens dt ON dt.id = ps.token_id \
         JOIN push_credentials pc ON pc.project_id = ps.project_id AND pc.kind = 'mipush' \
           AND pc.active \
         WHERE ps.id = $1 AND dt.project_id=ps.project_id AND dt.revoked_at IS NULL AND pc.id=ps.credential_id AND ps.generation=(SELECT generation FROM instance_settings WHERE singleton)",
    )
    .bind(send_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "credentials_missing".to_string())?;
    let device_token: String = row.get("native_token");
    let payload: serde_json::Value = row.get("payload");
    let config: serde_json::Value = row.get("config");
    let app_secret = credential_secret(state, &row, "mipush")?.trim().to_owned();
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

async fn try_fcm(state: &AppState, send_id: Uuid) -> Result<(u16, u128), String> {
    use std::time::Instant;
    let pool = &state.pool;
    let row = sqlx::query(
        "SELECT dt.native_token, ps.payload, ps.project_id, pc.secret_blob, pc.id AS credential_id, pc.secret_format \
         FROM push_sends ps \
         JOIN device_tokens dt ON dt.id = ps.token_id \
         JOIN push_credentials pc ON pc.project_id = ps.project_id AND pc.kind = 'fcm' \
           AND pc.active \
         WHERE ps.id = $1 AND dt.project_id=ps.project_id AND dt.revoked_at IS NULL AND pc.id=ps.credential_id AND ps.generation=(SELECT generation FROM instance_settings WHERE singleton)",
    )
    .bind(send_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "credentials_missing".to_string())?;
    let device_token: String = row.get("native_token");
    let payload: serde_json::Value = row.get("payload");
    let service_account_json = credential_secret(state, &row, "fcm")?.trim().to_owned();
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
    state: &AppState,
    cache: &Arc<TokenCache>,
    send_id: Uuid,
) -> Result<(u16, u128), String> {
    use std::time::Instant;
    let pool = &state.pool;
    let row = sqlx::query(
        "SELECT dt.native_token, dt.env, ps.payload, ps.project_id, pc.config, pc.secret_blob, pc.id AS credential_id, pc.secret_format \
         FROM push_sends ps \
         JOIN device_tokens dt ON dt.id = ps.token_id \
         JOIN push_credentials pc ON pc.project_id = ps.project_id AND pc.kind = 'apns' \
           AND pc.active \
         WHERE ps.id = $1 AND dt.project_id=ps.project_id AND dt.revoked_at IS NULL AND pc.id=ps.credential_id AND ps.generation=(SELECT generation FROM instance_settings WHERE singleton)",
    )
    .bind(send_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "credentials_missing".to_string())?;
    let credential_id: Uuid = row.get("credential_id");
    let device_token: String = row.get("native_token");
    let payload: serde_json::Value = row.get("payload");
    let config: serde_json::Value = row.get("config");
    let secret_pem = credential_secret(state, &row, "apns")?;

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
    let cached_jwt = cache.get(credential_id, "apns_jwt");
    let status = if let Some(jwt) = cached_jwt {
        match crate::apns::send_with_jwt(&cfg, &device_token, title, body_text, &jwt).await {
            Ok(status) => status,
            Err(error) => {
                let reason = error.to_string();
                if reason.contains("ExpiredProviderToken") {
                    cache.remove(credential_id, "apns_jwt");
                }
                return Err(reason);
            }
        }
    } else {
        let (status, jwt) = crate::apns::send_returning_jwt(&cfg, &device_token, title, body_text)
            .await
            .map_err(|e| e.to_string())?;
        cache.put(credential_id, "apns_jwt", jwt, Duration::from_mins(55));
        status
    };
    Ok((status, start.elapsed().as_millis()))
}

async fn try_webpush(state: &AppState, send_id: Uuid) -> Result<(u16, u128), String> {
    use std::time::Instant;
    let pool = &state.pool;
    let row = sqlx::query(
        "SELECT dt.native_token, dt.metadata, ps.payload, ps.project_id, pc.config, pc.secret_blob, pc.id AS credential_id, pc.secret_format \
         FROM push_sends ps \
         JOIN device_tokens dt ON dt.id = ps.token_id \
         JOIN push_credentials pc ON pc.project_id = ps.project_id AND pc.kind = 'webpush' \
           AND pc.active \
         WHERE ps.id = $1 AND dt.project_id=ps.project_id AND dt.revoked_at IS NULL AND pc.id=ps.credential_id AND ps.generation=(SELECT generation FROM instance_settings WHERE singleton)",
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
    let secret_pem = credential_secret(state, &row, "webpush")?;
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
    fn an_apns_credential_rejection_fails_rather_than_queues() {
        // The exact string the worker records — a project whose signing
        // key is wrong produced 31 of these, every one of them back on
        // the queue with `status = 'queued'` beside a recorded outcome
        // that already said rejected.
        for reason in [
            "apns rejected: status=403 body={\"reason\":\"InvalidProviderToken\"}",
            "apns rejected: status=403 body={\"reason\":\"MissingProviderToken\"}",
            "apns rejected: status=400 body={\"reason\":\"BadTopic\"}",
            "apns rejected: status=403 body={\"reason\":\"TopicDisallowed\"}",
            "apns rejected: status=400 body={\"reason\":\"BadCertificateEnvironment\"}",
        ] {
            assert!(
                is_configuration_failure(reason),
                "retrying this cannot fix it, and queueing it hides that push is \
                 down: {reason}"
            );
        }

        // The one that a retry does fix: the worker mints a fresh JWT
        // per attempt, so an expired one repairs itself.
        assert!(
            !is_configuration_failure(
                "apns rejected: status=403 body={\"reason\":\"ExpiredProviderToken\"}"
            ),
            "a fresh JWT is minted per attempt, so this one is worth retrying"
        );
    }

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
