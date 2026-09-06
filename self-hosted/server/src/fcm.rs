//! FCM (Firebase Cloud Messaging) vendor adapter — HTTP v1.
//!
//!   POST https://fcm.googleapis.com/v1/projects/<project_id>/messages:send
//!   Authorization: Bearer <oauth2 access token>
//!
//! The access token is minted from a service-account JSON: sign a
//! short-lived RS256 assertion, exchange it at Google's token
//! endpoint, cache what comes back until it nearly expires.
//!
//! ## Why this was rewritten
//!
//! It used to speak the legacy API — `POST /fcm/send` with an
//! `Authorization: key=<server key>` header — under a comment reading
//! "deprecated-but-still-working" and a note that v1 was a follow-up.
//! The comment was true when it was written. Google decommissioned
//! that endpoint, and it now answers **404** to everything:
//!
//! ```text
//! $ curl -i -X POST https://fcm.googleapis.com/fcm/send -H 'authorization: key=…'
//! HTTP/2 404
//! ```
//!
//! So every Android push this server has ever tried to send has
//! failed, and nothing said so, because this path had never been run
//! against real credentials — the one gap the release notes kept
//! calling "not yet verified". It was not merely unverified. It could
//! not work.
//!
//! ## Why the message carries `data` and not `notification`
//!
//! A `notification` message is rendered by the system before any app
//! code runs. Convenient, and it costs both of the things a reporting
//! SDK needs: the app's messaging service is never called, so nothing
//! observes delivery, and the tap opens the launcher intent with no
//! way to attribute it. insight measured exactly that — a tray entry
//! appeared, the app opened, and `onTap` never fired.
//!
//! A `data` message always reaches the SDK, which posts the
//! notification itself with its own pending intent. The tray entry
//! looks the same to the user and the tap comes back with the message
//! it belongs to.

use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const SCOPE: &str = "https://www.googleapis.com/auth/firebase.messaging";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

pub struct FcmConfig {
    /// The service-account JSON, verbatim, as uploaded by the
    /// operator. Parsed rather than pre-split so a bad paste fails
    /// with something readable instead of three empty strings.
    pub service_account_json: String,
}

/// The fields of a service-account JSON this needs. Google's file has
/// more; the rest is not our business.
#[derive(Deserialize)]
#[cfg_attr(test, derive(Debug))]
struct ServiceAccount {
    project_id: String,
    client_email: String,
    private_key: String,
}

#[derive(Debug, thiserror::Error)]
pub enum FcmError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("service account json: {0}")]
    Credentials(String),
    #[error("jwt: {0}")]
    Jwt(#[from] jsonwebtoken::errors::Error),
    #[error("oauth: status={status} body={body}")]
    OAuth { status: u16, body: String },
    #[error("fcm rejected: status={status} body={body}")]
    Rejected { status: u16, body: String },
}

#[derive(Serialize)]
struct AssertionClaims<'a> {
    iss: &'a str,
    scope: &'a str,
    aud: &'a str,
    exp: u64,
    iat: u64,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: Option<u64>,
}

/// One cached access token per process.
///
/// Google's tokens last an hour; minting one costs a network round
/// trip and an RSA signature, and a push worker sending a hundred
/// notifications would otherwise pay for both a hundred times.
/// Keyed by service account, because a multi-tenant deployment has
/// one per project.
static TOKEN_CACHE: Mutex<Vec<(String, String, Instant)>> = Mutex::new(Vec::new());

fn cached_token(key: &str) -> Option<String> {
    let cache = TOKEN_CACHE.lock().ok()?;
    cache.iter().find_map(|(k, token, expiry)| {
        if k == key && *expiry > Instant::now() {
            Some(token.clone())
        } else {
            None
        }
    })
}

fn store_token(key: &str, token: &str, ttl: Duration) {
    if let Ok(mut cache) = TOKEN_CACHE.lock() {
        cache.retain(|(k, _, expiry)| k != key && *expiry > Instant::now());
        cache.push((key.to_string(), token.to_string(), Instant::now() + ttl));
    }
}

fn parse(cfg: &FcmConfig) -> Result<ServiceAccount, FcmError> {
    let sa: ServiceAccount = serde_json::from_str(&cfg.service_account_json)
        .map_err(|e| FcmError::Credentials(e.to_string()))?;
    if sa.project_id.is_empty() || sa.client_email.is_empty() || sa.private_key.is_empty() {
        return Err(FcmError::Credentials(
            "project_id, client_email and private_key are all required".into(),
        ));
    }
    Ok(sa)
}

/// The project this credential publishes to. Used for the send URL
/// and, by the admin surface, to show the operator which Firebase
/// project they actually pasted.
pub fn project_id(cfg: &FcmConfig) -> Result<String, FcmError> {
    Ok(parse(cfg)?.project_id)
}

/// An OAuth access token minted from this credential.
///
/// Exposed for the credential probe, which needs to know whether
/// Google will mint one at all — that question is answered by this
/// call and by nothing the worker does before it starts sending.
pub async fn token_for(cfg: &FcmConfig) -> Result<String, FcmError> {
    access_token(&parse(cfg)?).await
}

async fn access_token(sa: &ServiceAccount) -> Result<String, FcmError> {
    if let Some(token) = cached_token(&sa.client_email) {
        return Ok(token);
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs();
    let claims = AssertionClaims {
        iss: &sa.client_email,
        scope: SCOPE,
        aud: TOKEN_URL,
        exp: now + 3600,
        iat: now,
    };
    let header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
    let key = jsonwebtoken::EncodingKey::from_rsa_pem(sa.private_key.as_bytes())?;
    let assertion = jsonwebtoken::encode(&header, &claims, &key)?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()?;
    let resp = client
        .post(TOKEN_URL)
        .form(&[
            ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
            ("assertion", &assertion),
        ])
        .send()
        .await?;
    let status = resp.status().as_u16();
    let body = resp.text().await.unwrap_or_default();
    if !(200..300).contains(&status) {
        return Err(FcmError::OAuth { status, body });
    }
    let parsed: TokenResponse = serde_json::from_str(&body).map_err(|e| FcmError::OAuth {
        status,
        body: format!("unreadable token response: {e}"),
    })?;

    // Expire ours a minute early. A token that dies between the check
    // and the send is a 401 on a notification somebody was waiting on.
    let ttl = Duration::from_secs(parsed.expires_in.unwrap_or(3600).saturating_sub(60));
    store_token(&sa.client_email, &parsed.access_token, ttl);
    Ok(parsed.access_token)
}

pub async fn send(
    cfg: &FcmConfig,
    device_token: &str,
    title: &str,
    body_text: &str,
) -> Result<u16, FcmError> {
    let sa = parse(cfg)?;
    let token = access_token(&sa).await?;

    // Every value in `data` must be a string — FCM rejects the message
    // outright otherwise, with an error about the wrong field.
    let message = serde_json::json!({
        "message": {
            "token": device_token,
            "data": {
                "sentori": "1",
                "title": title,
                "body": body_text,
            },
            "android": {
                // Without this a data-only message can be held until
                // the device next wakes, which for a crash alert is
                // the wrong trade.
                "priority": "HIGH",
            },
        }
    });

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()?;
    let resp = client
        .post(format!(
            "https://fcm.googleapis.com/v1/projects/{}/messages:send",
            sa.project_id
        ))
        .bearer_auth(&token)
        .json(&message)
        .send()
        .await?;
    let status = resp.status().as_u16();
    let body = resp.text().await.unwrap_or_default();
    if (200..300).contains(&status) {
        return Ok(status);
    }

    // v1 reports a dead token as UNREGISTERED / NOT_FOUND rather than
    // the legacy `results[0].error`. The quarantine logic reads the
    // message, so keep the vendor's own words in it.
    Err(FcmError::Rejected { status, body })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_paste_that_is_not_a_service_account_says_so() {
        let cfg = FcmConfig {
            service_account_json: "AAAA-legacy-server-key:APA91b…".into(),
        };
        assert!(
            matches!(parse(&cfg), Err(FcmError::Credentials(_))),
            "a legacy server key is what operators have today, and pasting it \
             has to produce a readable error rather than a 404 from Google"
        );
    }

    #[test]
    fn an_incomplete_service_account_is_refused_before_the_network() {
        let cfg = FcmConfig {
            service_account_json: r#"{"project_id":"p","client_email":"","private_key":"k"}"#
                .into(),
        };
        assert!(matches!(parse(&cfg), Err(FcmError::Credentials(_))));
    }

    #[test]
    fn a_real_shaped_service_account_parses() {
        let cfg = FcmConfig {
            service_account_json: r#"{
                "type": "service_account",
                "project_id": "qualcomm-insight",
                "private_key_id": "abc",
                "private_key": "-----BEGIN PRIVATE KEY-----\nMII…\n-----END PRIVATE KEY-----\n",
                "client_email": "firebase-adminsdk@qualcomm-insight.iam.gserviceaccount.com"
            }"#
            .into(),
        };
        assert_eq!(project_id(&cfg).ok().as_deref(), Some("qualcomm-insight"));
    }

    /// The cache must not hand one project's token to another.
    #[test]
    fn the_token_cache_is_keyed_by_account() {
        store_token("a@example.com", "token-a", Duration::from_mins(1));
        store_token("b@example.com", "token-b", Duration::from_mins(1));
        assert_eq!(cached_token("a@example.com").as_deref(), Some("token-a"));
        assert_eq!(cached_token("b@example.com").as_deref(), Some("token-b"));
        assert_eq!(cached_token("c@example.com"), None);
    }

    #[test]
    fn an_expired_token_is_not_returned() {
        store_token("old@example.com", "stale", Duration::from_secs(0));
        assert_eq!(cached_token("old@example.com"), None);
    }
}
