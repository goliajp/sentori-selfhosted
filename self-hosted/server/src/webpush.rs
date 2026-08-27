//! Minimal WebPush vendor adapter.
//!
//! Sends a no-payload "wake up" push to a WebPush subscription
//! endpoint. The browser SW receives an empty notification and
//! is expected to `event.waitUntil(self.registration.showNotification(...))`
//! after fetching the latest from /v1/push/receipts.
//!
//! Production-correct enough for the common SaaS use case. Full
//! encrypted-payload WebPush (RFC 8030 + 8188 + 8291) is K7.3
//! follow-up.

#![allow(dead_code)]

use base64::{Engine, engine::general_purpose};
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// The `vapid_` prefix is part of the VAPID spec vocabulary; dropping
// it would make these fields ambiguous against the other key material
// in this module.
#[allow(clippy::struct_field_names)]
pub struct WebPushConfig {
    pub vapid_subject: String,
    pub vapid_public_key_b64url: String,
    pub vapid_private_pem: String,
}

#[derive(Debug, thiserror::Error)]
pub enum WebPushError {
    #[error("jwt sign: {0}")]
    Jwt(#[from] jsonwebtoken::errors::Error),
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("bad endpoint url: {0}")]
    BadUrl(String),
    #[error("provider rejected: status={status} body={body}")]
    Rejected { status: u16, body: String },
}

#[derive(Serialize)]
struct VapidClaims {
    aud: String,
    exp: u64,
    sub: String,
}

/// Subscription endpoint URL — what the browser handed the SDK
/// back from PushManager.subscribe(). Includes the host and
/// usually a query-string subscription id.
#[derive(Deserialize)]
pub struct WebPushSubscription {
    pub endpoint: String,
}

/// Send a no-payload wake push.
///
/// Returns the HTTP status code; production code maps 201 → sent,
/// 410/404 → quarantine token, others → retry.
pub async fn send(
    cfg: &WebPushConfig,
    sub_endpoint: &str,
    ttl_sec: u32,
) -> Result<u16, WebPushError> {
    send_with_payload(cfg, sub_endpoint, ttl_sec, None).await
}

/// When `payload` is set with the subscription's `p256dh` +
/// `auth_secret`, encrypt the payload per RFC 8291 + 8188 and
/// POST it (browser will receive title/body). When None, sends a
/// no-payload wake push (SW fires with no body — backwards compat).
pub async fn send_with_payload(
    cfg: &WebPushConfig,
    sub_endpoint: &str,
    ttl_sec: u32,
    payload: Option<(&[u8], &str, &str)>,
) -> Result<u16, WebPushError> {
    let url = url::Url::parse(sub_endpoint).map_err(|e| WebPushError::BadUrl(e.to_string()))?;
    let aud = format!("{}://{}", url.scheme(), url.host_str().unwrap_or(""));

    // VAPID JWT (ES256). exp = now + 12h (RFC 8292 ≤ 24h)
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs();
    let claims = VapidClaims {
        aud,
        exp: now + 12 * 3600,
        sub: cfg.vapid_subject.clone(),
    };
    let header = Header::new(Algorithm::ES256);
    let key = EncodingKey::from_ec_pem(cfg.vapid_private_pem.as_bytes())?;
    let jwt = jsonwebtoken::encode(&header, &claims, &key)?;

    let auth_header = format!("vapid t={jwt}, k={}", cfg.vapid_public_key_b64url);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()?;
    let mut req = client
        .post(sub_endpoint)
        .header("Authorization", auth_header)
        .header("TTL", ttl_sec.to_string());

    if let Some((plain, p256dh, auth_secret)) = payload {
        let enc = crate::webpush_encrypt::encrypt(plain, p256dh, auth_secret).map_err(|e| {
            WebPushError::Rejected {
                status: 0,
                body: format!("encrypt: {e}"),
            }
        })?;
        req = req
            .header("Content-Encoding", enc.content_encoding)
            .header("Content-Length", enc.body.len().to_string())
            .body(enc.body);
    } else {
        req = req.header("Content-Length", "0");
    }

    let resp = req.send().await?;
    let status = resp.status().as_u16();
    if !(200..400).contains(&status) {
        let body = resp.text().await.unwrap_or_default();
        return Err(WebPushError::Rejected { status, body });
    }
    Ok(status)
}

/// Helper used by callers that need to base64url-encode a binary
/// EC pub key on the way in.
#[must_use]
pub fn b64url(bytes: &[u8]) -> String {
    general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}
