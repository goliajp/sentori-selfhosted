//! Outbound webhook delivery for alert channels.
//!
//! When an alert rule's channels include a `webhook` entry with
//! `{ url, secret? }`, POST the payload there with an
//! `X-Sentori-Signature: <hex(HMAC-SHA256(secret, body))>` header.
//!
//! Used by Slack-compatible incoming-webhook URLs, Discord
//! webhooks, custom infra. For Slack-the-product specifically,
//! use `kind=slack` with their slash command shape — TODO.

#![allow(dead_code)]

use std::time::Duration;

use hmac::{Hmac, Mac};
use serde::Serialize;
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, thiserror::Error)]
pub enum WebhookError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("webhook rejected: status={0}")]
    Rejected(u16),
    /// Unreachable in practice — HMAC-SHA256 accepts a key of any
    /// length — but modelled as an error so the signing path on a
    /// request-reachable code path never panics.
    #[error("invalid webhook signing key: {0}")]
    InvalidSigningKey(#[from] hmac::digest::InvalidLength),
}

pub async fn deliver<T: Serialize>(
    url: &str,
    secret: Option<&str>,
    payload: &T,
) -> Result<u16, WebhookError> {
    let body = serde_json::to_vec(payload).unwrap_or_default();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;
    let mut req = client
        .post(url)
        .header("content-type", "application/json")
        .header("user-agent", "sentori/0.2 webhook");
    if let Some(s) = secret {
        let mut mac = HmacSha256::new_from_slice(s.as_bytes())?;
        mac.update(&body);
        let sig = hex::encode(mac.finalize().into_bytes());
        req = req.header("x-sentori-signature", sig);
    }
    let resp = req.body(body).send().await?;
    let status = resp.status().as_u16();
    if (200..400).contains(&status) {
        Ok(status)
    } else {
        Err(WebhookError::Rejected(status))
    }
}
