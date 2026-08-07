//! Typed errors for [`crate::IntegrationService`] and
//! [`crate::IntegrationAdapter`] implementations.

use thiserror::Error;
use uuid::Uuid;

/// Failure modes shared across the trait surface + service.
#[derive(Debug, Error)]
pub enum IntegrationError {
    /// Adapter / kind not recognised, or required env vars
    /// unset (covers `is_configured() = false`).
    #[error("integration not configured: {0}")]
    NotConfigured(String),

    /// Caller-provided input failed validation
    /// (`accept_manual_config` got a malformed form,
    /// `create_issue` missing context field, etc.).
    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// OAuth handshake / state issues.
    #[error("oauth: {0}")]
    OAuth(String),

    /// Upstream API rejected us (4xx / 5xx / malformed
    /// response).
    #[error("upstream: {0}")]
    Upstream(String),

    /// HTTP transport failure (DNS / TLS / timeout) — a
    /// retry on the next tick might succeed.
    #[error("http transport: {0}")]
    HttpTransport(String),

    /// Issue not linked to this kind of integration —
    /// `update_status` can't proceed without a prior link.
    #[error("issue {issue_id} not linked to {kind}")]
    NotLinked {
        /// The Sentori issue id.
        issue_id: Uuid,
        /// The integration kind looked up.
        kind: String,
    },

    /// Project FK violation on `store_config`.
    #[error("project {0} not found")]
    ProjectNotFound(Uuid),

    /// Issue FK violation on `record_link`.
    #[error("issue {0} not found")]
    IssueNotFound(Uuid),

    /// No adapter registered for the requested `kind`.
    #[error("no adapter registered for kind {0:?}")]
    NoAdapter(String),

    /// Database error.
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
}

impl IntegrationError {
    /// True for variants safe to surface to the dashboard
    /// user (vs operator-only infra errors).
    #[must_use]
    pub const fn is_safe_for_end_user(&self) -> bool {
        matches!(
            self,
            Self::NotConfigured(_)
                | Self::InvalidInput(_)
                | Self::OAuth(_)
                | Self::NotLinked { .. }
                | Self::ProjectNotFound(_)
                | Self::IssueNotFound(_)
                | Self::NoAdapter(_)
                | Self::Upstream(_)
        )
    }
}

impl From<reqwest::Error> for IntegrationError {
    fn from(err: reqwest::Error) -> Self {
        Self::HttpTransport(err.to_string())
    }
}
