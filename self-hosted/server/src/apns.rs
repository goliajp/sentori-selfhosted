//! Minimal APNs (Apple Push Notification service) vendor adapter
//! over HTTP/2 with token-based authentication.
//!
//! Token-auth JWT is ES256-signed with the `.p8` EC private key
//! the developer downloads from Apple Developer. The same JWT can
//! be reused for up to an hour across many sends, but we mint a
//! fresh one per call here for simplicity. Caching is a follow-up.

#![allow(dead_code)]

use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub struct ApnsConfig {
    /// Apple Team ID (10 chars) — Developer portal.
    pub team_id: String,
    /// Auth key ID (10 chars) — from the .p8 filename suffix.
    pub key_id: String,
    /// Bundle id (apns-topic), e.g. com.example.myapp
    pub topic: String,
    /// .p8 EC private key contents (PEM).
    pub private_pem: String,
    /// Sandbox vs production. Default true for production.
    pub production: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ApnsError {
    #[error("jwt sign: {0}")]
    Jwt(#[from] jsonwebtoken::errors::Error),
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("apns rejected: status={status} body={body}")]
    Rejected { status: u16, body: String },
}

#[derive(Serialize)]
struct ApnsJwtClaims {
    iss: String,
    iat: u64,
}

#[derive(Serialize)]
struct ApsAlert<'a> {
    title: &'a str,
    body: &'a str,
}

#[derive(Serialize)]
struct ApsBody<'a> {
    aps: Aps<'a>,
}

#[derive(Serialize)]
struct Aps<'a> {
    alert: ApsAlert<'a>,
}

#[derive(Deserialize)]
struct ApnsErrorBody {
    #[allow(dead_code)]
    reason: Option<String>,
}

pub fn mint_jwt(cfg: &ApnsConfig) -> Result<String, ApnsError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs();
    let mut header = Header::new(Algorithm::ES256);
    header.kid = Some(cfg.key_id.clone());
    let claims = ApnsJwtClaims {
        iss: cfg.team_id.clone(),
        iat: now,
    };
    let key = EncodingKey::from_ec_pem(cfg.private_pem.as_bytes())?;
    Ok(jsonwebtoken::encode(&header, &claims, &key)?)
}

pub async fn send_with_jwt(
    cfg: &ApnsConfig,
    device_token: &str,
    title: &str,
    body_text: &str,
    jwt: &str,
) -> Result<u16, ApnsError> {
    do_send(cfg, device_token, title, body_text, jwt).await
}

pub async fn send_returning_jwt(
    cfg: &ApnsConfig,
    device_token: &str,
    title: &str,
    body_text: &str,
) -> Result<(u16, String), ApnsError> {
    let jwt = mint_jwt(cfg)?;
    let status = do_send(cfg, device_token, title, body_text, &jwt).await?;
    Ok((status, jwt))
}

pub async fn send(
    cfg: &ApnsConfig,
    device_token: &str,
    title: &str,
    body_text: &str,
) -> Result<u16, ApnsError> {
    let jwt = mint_jwt(cfg)?;
    do_send(cfg, device_token, title, body_text, &jwt).await
}

async fn do_send(
    cfg: &ApnsConfig,
    device_token: &str,
    title: &str,
    body_text: &str,
    jwt: &str,
) -> Result<u16, ApnsError> {
    let host = if cfg.production {
        "https://api.push.apple.com"
    } else {
        "https://api.sandbox.push.apple.com"
    };
    let url = format!("{host}/3/device/{device_token}");
    let payload = ApsBody {
        aps: Aps {
            alert: ApsAlert {
                title,
                body: body_text,
            },
        },
    };
    let client = reqwest::Client::builder()
        .http2_prior_knowledge()
        .timeout(Duration::from_secs(15))
        .build()?;
    let resp = client
        .post(&url)
        .header("authorization", format!("bearer {jwt}"))
        .header("apns-topic", &cfg.topic)
        .header("apns-push-type", "alert")
        .json(&payload)
        .send()
        .await?;
    let status = resp.status().as_u16();
    if status == 200 {
        return Ok(status);
    }
    let text = resp.text().await.unwrap_or_default();
    Err(ApnsError::Rejected { status, body: text })
}
