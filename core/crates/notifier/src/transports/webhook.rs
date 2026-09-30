//! [`WebhookTransport`] — generic outbound HTTP POST.
//!
//! Payload shape: `{"subject":..., "body":..., "metadata":
//! ...}`. Vendor adapters (Slack Block Kit, Linear GraphQL,
//! etc.) live in K12 and shape their own payload via
//! `Notification.body` (already JSON-stringified) +
//! `Notification.metadata`. K11 is intentionally
//! vendor-agnostic at the transport layer.

use std::time::Duration;

use async_trait::async_trait;
use serde_json::json;

use crate::error::TransportError;
use crate::model::{Channel, Notification};
use crate::transports::Notifier;

/// Default per-call timeout. Webhook receivers should ack
/// fast; if they hang we'd rather retry next tick than
/// block the dispatcher.
pub const DEFAULT_HTTP_TIMEOUT_SECS: u64 = 10;

/// Webhook transport. Cheap to clone (`reqwest::Client` is
/// `Arc`-backed).
#[derive(Clone, Debug)]
pub struct WebhookTransport {
    client: reqwest::Client,
}

impl WebhookTransport {
    /// Construct with sane defaults.
    ///
    /// # Panics
    ///
    /// Underlying [`reqwest::Client::builder`] only fails on
    /// system TLS misconfiguration; we panic rather than
    /// fail-soft since the transport is useless without HTTP.
    #[must_use]
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(DEFAULT_HTTP_TIMEOUT_SECS))
            .user_agent(format!(
                "sentori-notifier/{} (+https://sentori.golia.jp)",
                env!("CARGO_PKG_VERSION"),
            ))
            .build()
            .expect("reqwest client builder must succeed");
        Self { client }
    }

    /// Swap the client (shared pool tuning by consumer).
    #[must_use]
    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }

    /// Borrow the client.
    #[must_use]
    pub const fn client(&self) -> &reqwest::Client {
        &self.client
    }
}

impl Default for WebhookTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Notifier for WebhookTransport {
    fn channel(&self) -> Channel {
        Channel::Webhook
    }

    async fn send(&self, n: &Notification) -> Result<(), TransportError> {
        let payload = json!({
            "subject": n.subject,
            "body": n.body,
            "metadata": n.metadata,
        });
        let resp = self
            .client
            .post(&n.recipient)
            .json(&payload)
            .send()
            .await
            .map_err(|e| TransportError::Webhook(format!("http transport: {e}")))?;
        if !resp.status().is_success() {
            return Err(TransportError::Webhook(format!(
                "non-2xx status {} from {}",
                resp.status(),
                n.recipient,
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn channel_is_webhook() {
        assert_eq!(WebhookTransport::new().channel(), Channel::Webhook);
    }

    #[test]
    fn default_constructs() {
        let _t = WebhookTransport::default();
    }
}
