//! HCM (Huawei Cloud Messaging) vendor adapter.
//!
//! Uses OAuth2 token-based auth: client_id + client_secret → bearer
//! token (cached for ~1h) → POST to push-api.cloud.huawei.com.
//!
//! v0.2 ships the OAuth exchange inline per send (no cache).
//! Caching is a follow-up.

#![allow(dead_code)]

use serde::Serialize;
use std::time::Duration;

pub struct HcmConfig {
    pub client_id: String,
    pub client_secret: String,
    /// Huawei app id for the push endpoint path.
    pub app_id: String,
}

#[derive(Debug, thiserror::Error)]
pub enum HcmError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("hcm rejected: status={status} body={body}")]
    Rejected { status: u16, body: String },
    #[error("oauth missing access_token")]
    NoAccessToken,
}

#[derive(Serialize)]
struct PushBody<'a> {
    validate_only: bool,
    message: Message<'a>,
}

#[derive(Serialize)]
struct Message<'a> {
    token: Vec<&'a str>,
    notification: Notification<'a>,
    android: AndroidConfig,
}

#[derive(Serialize)]
struct Notification<'a> {
    title: &'a str,
    body: &'a str,
}

#[derive(Serialize)]
struct AndroidConfig {
    collapse_key: i32,
    urgency: &'static str,
    ttl: &'static str,
}

pub async fn fetch_oauth_token(cfg: &HcmConfig) -> Result<String, HcmError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()?;
    let oauth: serde_json::Value = client
        .post("https://oauth-login.cloud.huawei.com/oauth2/v3/token")
        .form(&[
            ("grant_type", "client_credentials"),
            ("client_id", cfg.client_id.as_str()),
            ("client_secret", cfg.client_secret.as_str()),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let token = oauth["access_token"]
        .as_str()
        .ok_or(HcmError::NoAccessToken)?
        .to_string();
    Ok(token)
}

pub async fn send_with_token(
    cfg: &HcmConfig,
    token: &str,
    device_token: &str,
    title: &str,
    body_text: &str,
) -> Result<u16, HcmError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()?;
    do_send(&client, cfg, token, device_token, title, body_text).await
}

pub async fn send(
    cfg: &HcmConfig,
    device_token: &str,
    title: &str,
    body_text: &str,
) -> Result<u16, HcmError> {
    let token = fetch_oauth_token(cfg).await?;
    send_with_token(cfg, &token, device_token, title, body_text).await
}

async fn do_send(
    client: &reqwest::Client,
    cfg: &HcmConfig,
    token: &str,
    device_token: &str,
    title: &str,
    body_text: &str,
) -> Result<u16, HcmError> {
    let url = format!(
        "https://push-api.cloud.huawei.com/v1/{}/messages:send",
        cfg.app_id
    );
    let resp = client
        .post(&url)
        .header("authorization", format!("Bearer {token}"))
        .json(&PushBody {
            validate_only: false,
            message: Message {
                token: vec![device_token],
                notification: Notification {
                    title,
                    body: body_text,
                },
                android: AndroidConfig {
                    collapse_key: -1,
                    urgency: "HIGH",
                    ttl: "1d",
                },
            },
        })
        .send()
        .await?;
    let status = resp.status().as_u16();
    if (200..400).contains(&status) {
        return Ok(status);
    }
    let body = resp.text().await.unwrap_or_default();
    Err(HcmError::Rejected { status, body })
}
