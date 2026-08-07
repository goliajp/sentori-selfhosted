//! Slack incoming-webhook reference adapter.
//!
//! Slack is `ConnectMode::Manual`: the user pastes an
//! incoming-webhook URL into the dashboard form rather than
//! going through OAuth. That makes Slack the smallest
//! complete adapter — perfect K12 trait validator.
//!
//! Vendor specifics:
//! - **Config shape**: `{"webhook_url": "https://hooks.slack.com/services/T.../B.../..."}`.
//! - **create_issue**: POST `{text: ..., blocks: [...]}` to
//!   webhook_url. Slack assigns no first-class issue id; the
//!   adapter returns the webhook_url + a synthetic id so
//!   `update_status` can post a follow-up.
//! - **update_status**: POST another message referencing the
//!   prior `external_id` (which is the issue's dashboard URL
//!   in our convention, since Slack doesn't return a stable
//!   message ts for incoming webhooks).
//!
//! The HTTP client base URL is injectable via
//! [`SlackAdapter::with_client`] so the integration test
//! drives a local mock instead of the real Slack endpoint.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::error::IntegrationError;
use crate::model::{ConnectMode, ExternalRef, IssueContext, IssueLifecycleEvent};
use crate::traits::IntegrationAdapter;

/// Stable kind id.
pub const KIND: &str = "slack";

/// Persisted JSON shape for `integrations.config`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlackConfig {
    /// Incoming webhook URL (`https://hooks.slack.com/...`).
    pub webhook_url: String,
}

/// Slack adapter. Cheap to clone (`reqwest::Client` is
/// `Arc`-backed).
#[derive(Debug, Clone)]
pub struct SlackAdapter {
    client: reqwest::Client,
    /// When non-empty, REPLACES the host part of the
    /// `webhook_url` from config — lets integration tests
    /// point at a local mock without changing stored config.
    override_base: Option<String>,
}

impl SlackAdapter {
    /// Build with the default HTTP client.
    #[must_use]
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent(format!(
                    "sentori-integration-slack/{}",
                    env!("CARGO_PKG_VERSION")
                ))
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("reqwest client must build"),
            override_base: None,
        }
    }

    /// Swap in a pre-built client (consumer might pool one).
    #[must_use]
    pub fn with_client(mut self, client: reqwest::Client) -> Self {
        self.client = client;
        self
    }

    /// Override the URL host. Test injection — production
    /// callers leave this as default.
    #[must_use]
    pub fn with_base_url(mut self, base: impl Into<String>) -> Self {
        self.override_base = Some(base.into());
        self
    }

    /// Resolve the effective webhook URL — applies test
    /// `override_base` if set.
    fn effective_url(&self, stored: &str) -> String {
        let Some(base) = self.override_base.as_deref() else {
            return stored.to_string();
        };
        // Replace scheme+host of the stored URL with the
        // override. Path is preserved so the mock server can
        // assert it.
        let trimmed_base = base.trim_end_matches('/');
        if let Some(path_start) = stored.find("//").and_then(|i| stored[i + 2..].find('/')) {
            let path = &stored[stored.find("//").map(|i| i + 2 + path_start).unwrap_or(0)..];
            format!("{trimmed_base}{path}")
        } else {
            // No path in the stored URL — use the override
            // verbatim.
            trimmed_base.to_string()
        }
    }

    /// Build the create_issue payload (Block Kit-flavoured
    /// minimal shape).
    fn build_create_payload(ctx: &IssueContext) -> Value {
        let title = format!("[{}] {}", ctx.error_type, ctx.error_message);
        let detail = format!(
            "*Release*: {release}\n*Env*: {env}\n*Events*: {n}\n<{url}|Open in Sentori>",
            release = ctx.release,
            env = ctx.environment,
            n = ctx.event_count,
            url = ctx.url,
        );
        json!({
            "text": title.clone(),
            "blocks": [
                {
                    "type": "section",
                    "text": { "type": "mrkdwn", "text": format!("*{title}*") }
                },
                {
                    "type": "section",
                    "text": { "type": "mrkdwn", "text": detail }
                }
            ]
        })
    }

    /// Build the update_status payload (single section,
    /// references the original issue URL).
    fn build_update_payload(external_id: &str, event: IssueLifecycleEvent) -> Value {
        let badge = match event {
            IssueLifecycleEvent::Created => ":new:",
            IssueLifecycleEvent::Regressed => ":warning:",
            IssueLifecycleEvent::Resolved => ":white_check_mark:",
        };
        let verb = match event {
            IssueLifecycleEvent::Created => "Created",
            IssueLifecycleEvent::Regressed => "Regressed",
            IssueLifecycleEvent::Resolved => "Resolved",
        };
        json!({
            "text": format!("{badge} {verb}: {external_id}"),
        })
    }
}

impl Default for SlackAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl IntegrationAdapter for SlackAdapter {
    fn kind(&self) -> &'static str {
        KIND
    }

    fn is_configured(&self) -> bool {
        // Slack is always "configured" at the adapter level
        // — per-project webhook URL is required at config
        // store time, not at adapter init.
        true
    }

    fn connect_mode(&self) -> ConnectMode {
        ConnectMode::Manual
    }

    fn oauth_authorise_url(&self, _state: &str, _redirect_uri: &str) -> String {
        // Manual mode — no OAuth URL. Empty string sentinel.
        String::new()
    }

    async fn exchange_code(
        &self,
        _code: &str,
        _redirect_uri: &str,
    ) -> Result<Value, IntegrationError> {
        Err(IntegrationError::InvalidInput(
            "slack uses Manual connect mode; call accept_manual_config".into(),
        ))
    }

    async fn accept_manual_config(&self, form: Value) -> Result<Value, IntegrationError> {
        let parsed: SlackConfig = serde_json::from_value(form)
            .map_err(|e| IntegrationError::InvalidInput(format!("slack form: {e}")))?;
        if !parsed.webhook_url.starts_with("https://hooks.slack.com/")
            && !parsed.webhook_url.starts_with("http://")
        // http:// allowed for tests
        {
            return Err(IntegrationError::InvalidInput(
                "webhook_url must be a Slack incoming-webhook URL".into(),
            ));
        }
        Ok(serde_json::to_value(parsed)
            .map_err(|e| IntegrationError::InvalidInput(format!("encode: {e}")))?)
    }

    async fn create_issue(
        &self,
        config: &Value,
        ctx: &IssueContext,
    ) -> Result<ExternalRef, IntegrationError> {
        let parsed: SlackConfig = serde_json::from_value(config.clone())
            .map_err(|e| IntegrationError::InvalidInput(format!("slack config: {e}")))?;
        let url = self.effective_url(&parsed.webhook_url);
        let payload = Self::build_create_payload(ctx);
        let resp = self.client.post(&url).json(&payload).send().await?;
        if !resp.status().is_success() {
            return Err(IntegrationError::Upstream(format!(
                "slack non-2xx {} for create_issue",
                resp.status()
            )));
        }
        // Slack incoming webhooks return "ok" and assign no
        // stable id. We synthesise external_id from the
        // Sentori issue URL so update_status references
        // something humans recognise.
        Ok(ExternalRef {
            external_id: ctx.url.clone(),
            external_url: ctx.url.clone(),
        })
    }

    async fn update_status(
        &self,
        config: &Value,
        external_id: &str,
        event: IssueLifecycleEvent,
    ) -> Result<(), IntegrationError> {
        let parsed: SlackConfig = serde_json::from_value(config.clone())
            .map_err(|e| IntegrationError::InvalidInput(format!("slack config: {e}")))?;
        let url = self.effective_url(&parsed.webhook_url);
        let payload = Self::build_update_payload(external_id, event);
        let resp = self.client.post(&url).json(&payload).send().await?;
        if !resp.status().is_success() {
            return Err(IntegrationError::Upstream(format!(
                "slack non-2xx {} for update_status",
                resp.status()
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use sentori_workspace_identity::ProjectId;
    use uuid::Uuid;

    fn ctx() -> IssueContext {
        IssueContext {
            issue_id: Uuid::now_v7(),
            project_id: ProjectId::new(),
            error_type: "TypeError".into(),
            error_message: "x is undefined".into(),
            release: "app@1.0.0".into(),
            environment: "production".into(),
            url: "https://sentori.example.com/issues/abc".into(),
            event_count: 17,
            crash_site: None,
        }
    }

    #[test]
    fn kind_and_mode() {
        let a = SlackAdapter::new();
        assert_eq!(a.kind(), "slack");
        assert_eq!(a.connect_mode(), ConnectMode::Manual);
        assert!(a.is_configured());
    }

    #[test]
    fn create_payload_has_title_and_link() {
        let payload = SlackAdapter::build_create_payload(&ctx());
        let s = payload.to_string();
        assert!(s.contains("TypeError"));
        assert!(s.contains("x is undefined"));
        assert!(s.contains("app@1.0.0"));
        assert!(s.contains("Open in Sentori"));
    }

    #[test]
    fn update_payload_badge_per_event() {
        let p_created = SlackAdapter::build_update_payload("X", IssueLifecycleEvent::Created);
        let p_reg = SlackAdapter::build_update_payload("X", IssueLifecycleEvent::Regressed);
        let p_res = SlackAdapter::build_update_payload("X", IssueLifecycleEvent::Resolved);
        assert!(p_created.to_string().contains(":new:"));
        assert!(p_reg.to_string().contains(":warning:"));
        assert!(p_res.to_string().contains(":white_check_mark:"));
    }

    #[tokio::test]
    async fn accept_manual_config_round_trip() {
        let a = SlackAdapter::new();
        let form = json!({"webhookUrl": "https://hooks.slack.com/services/T/B/abcdef"});
        let stored = a.accept_manual_config(form).await.unwrap();
        assert_eq!(
            stored["webhookUrl"],
            "https://hooks.slack.com/services/T/B/abcdef"
        );
    }

    #[tokio::test]
    async fn accept_manual_config_rejects_non_slack_url() {
        let a = SlackAdapter::new();
        let form = json!({"webhookUrl": "ftp://elsewhere"});
        assert!(matches!(
            a.accept_manual_config(form).await,
            Err(IntegrationError::InvalidInput(_))
        ));
    }

    #[tokio::test]
    async fn exchange_code_errors_with_helpful_message() {
        let a = SlackAdapter::new();
        let err = a.exchange_code("c", "r").await.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Manual"));
    }

    #[test]
    fn oauth_authorise_url_is_empty() {
        let a = SlackAdapter::new();
        assert!(a.oauth_authorise_url("state", "uri").is_empty());
    }

    #[test]
    fn effective_url_swaps_base_keeps_path() {
        let a = SlackAdapter::new().with_base_url("http://127.0.0.1:9000");
        let out = a.effective_url("https://hooks.slack.com/services/T/B/abc");
        assert_eq!(out, "http://127.0.0.1:9000/services/T/B/abc");
    }

    #[test]
    fn effective_url_passthrough_when_no_override() {
        let a = SlackAdapter::new();
        let stored = "https://hooks.slack.com/services/T/B/abc";
        assert_eq!(a.effective_url(stored), stored);
    }
}
