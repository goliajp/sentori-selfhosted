//! The [`IntegrationAdapter`] trait that every vendor
//! implementation (K12 Slack + K12.1-K12.4 follow-ups)
//! satisfies.

use async_trait::async_trait;
use serde_json::Value;

use crate::error::IntegrationError;
use crate::model::{ConnectMode, ExternalRef, IssueContext, IssueLifecycleEvent};

/// The contract that every vendor adapter implements.
///
/// `dyn`-safe via `async_trait` so
/// [`crate::IntegrationService`] can hold
/// `Arc<dyn IntegrationAdapter>` in a `HashMap<kind, _>`.
#[async_trait]
pub trait IntegrationAdapter: Send + Sync + std::fmt::Debug {
    /// Lowercase stable identifier — used as the
    /// `integrations.kind` column value and the
    /// `/v1/integrations/<kind>/...` URL path segment.
    ///
    /// MUST be a compile-time constant (`&'static str`) so
    /// the runtime registry never holds an owned string.
    fn kind(&self) -> &'static str;

    /// Adapter has credentials / env vars to operate.
    /// Returning `false` makes
    /// [`crate::IntegrationService::dispatch`] skip the
    /// adapter with reason `"not configured"`.
    fn is_configured(&self) -> bool;

    /// How this adapter is connected. Default = OAuth.
    fn connect_mode(&self) -> ConnectMode {
        ConnectMode::OAuth
    }

    /// Build the OAuth authorise URL the user is redirected
    /// to. `state` is the CSRF token already minted by the
    /// caller / api layer. `redirect_uri` is the
    /// post-callback URL.
    ///
    /// Adapters with `connect_mode() == Manual` return an
    /// empty string — the dispatcher skips them.
    fn oauth_authorise_url(&self, state: &str, redirect_uri: &str) -> String;

    /// Exchange an OAuth `code` for whatever JSONB blob the
    /// vendor's API returns (typically `{access_token,
    /// refresh_token, scope, expires_in, …}`). The api
    /// layer persists this verbatim into
    /// `integrations.config`.
    ///
    /// # Errors
    ///
    /// [`IntegrationError::OAuth`] when the vendor rejects;
    /// [`IntegrationError::Upstream`] when the response
    /// is malformed.
    async fn exchange_code(
        &self,
        code: &str,
        redirect_uri: &str,
    ) -> Result<Value, IntegrationError>;

    /// Manual-mode equivalent of `exchange_code`: take a
    /// JSON form payload, validate, return the
    /// `integrations.config` blob to persist.
    ///
    /// Default impl returns
    /// [`IntegrationError::InvalidInput`] so OAuth-only
    /// adapters don't pretend to support manual flow.
    ///
    /// # Errors
    ///
    /// [`IntegrationError::InvalidInput`] for unknown /
    /// malformed forms.
    async fn accept_manual_config(&self, _form: Value) -> Result<Value, IntegrationError> {
        Err(IntegrationError::InvalidInput(format!(
            "{} doesn't support manual config",
            self.kind(),
        )))
    }

    /// Create the upstream item for a Sentori issue.
    /// Returns the [`ExternalRef`] persisted in
    /// `issue_integration_links` so subsequent
    /// `update_status` calls target the right thread.
    ///
    /// # Errors
    ///
    /// Adapter-specific. Common: [`IntegrationError::Upstream`],
    /// [`IntegrationError::HttpTransport`].
    async fn create_issue(
        &self,
        config: &Value,
        ctx: &IssueContext,
    ) -> Result<ExternalRef, IntegrationError>;

    /// React to a Sentori issue lifecycle transition.
    /// `external_id` is the upstream item id resolved from
    /// `issue_integration_links` by the service before
    /// calling. Adapter decides what `Resolved` /
    /// `Regressed` maps to upstream (Linear: close + comment
    /// vs reopen; Slack: thread reply).
    ///
    /// # Errors
    ///
    /// Adapter-specific.
    async fn update_status(
        &self,
        config: &Value,
        external_id: &str,
        event: IssueLifecycleEvent,
    ) -> Result<(), IntegrationError>;
}
