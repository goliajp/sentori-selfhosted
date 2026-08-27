//! MiPush (Xiaomi) vendor adapter.
//!
//! Uses app-secret-based auth header:
//!   Authorization: key=<app_secret>
//!
//! POST to https://api.xmpush.xiaomi.com/v3/message/regid

#![allow(dead_code)]

use std::time::Duration;

pub struct MiPushConfig {
    pub app_secret: String,
    pub package_name: String,
}

#[derive(Debug, thiserror::Error)]
pub enum MiPushError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("mipush rejected: status={status} body={body}")]
    Rejected { status: u16, body: String },
}

pub async fn send(
    cfg: &MiPushConfig,
    device_token: &str,
    title: &str,
    body_text: &str,
) -> Result<u16, MiPushError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()?;
    let mut form = vec![
        ("registration_id", device_token.to_string()),
        ("payload", body_text.to_string()),
        ("title", title.to_string()),
        ("description", body_text.to_string()),
        ("restricted_package_name", cfg.package_name.clone()),
        ("notify_type", "-1".to_string()),
        ("pass_through", "0".to_string()),
    ];
    form.shrink_to_fit();
    let resp = client
        .post("https://api.xmpush.xiaomi.com/v3/message/regid")
        .header("authorization", format!("key={}", cfg.app_secret))
        .form(&form)
        .send()
        .await?;
    let status = resp.status().as_u16();
    if (200..400).contains(&status) {
        return Ok(status);
    }
    let body = resp.text().await.unwrap_or_default();
    Err(MiPushError::Rejected { status, body })
}
