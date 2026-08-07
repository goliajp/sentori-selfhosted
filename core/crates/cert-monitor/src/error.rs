//! Typed error for [`crate::CertMonitor`].

use thiserror::Error;
use uuid::Uuid;

/// Failure modes for cert-monitor.
#[derive(Debug, Error)]
pub enum CertMonitorError {
    /// Project FK violation on add_watch.
    #[error("project {0} not found")]
    ProjectNotFound(Uuid),

    /// Caller passed an empty / oversized / non-ASCII domain
    /// string.
    #[error("invalid domain: {0}")]
    InvalidDomain(String),

    /// HTTP client failed (timeout, DNS, TLS, etc.).
    #[error("http transport: {0}")]
    HttpTransport(String),

    /// crt.sh returned non-2xx.
    #[error("crt.sh {status} for {domain}")]
    UpstreamStatus {
        /// HTTP status code (e.g. 502).
        status: u16,
        /// Which watched domain triggered the response.
        domain: String,
    },

    /// crt.sh returned a payload K10 couldn't parse.
    #[error("malformed crt.sh response: {0}")]
    MalformedResponse(String),

    /// Database error.
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
}

impl CertMonitorError {
    /// True for variants safe to render to the dashboard
    /// verbatim. False for infra-only variants.
    #[must_use]
    pub const fn is_safe_for_end_user(&self) -> bool {
        matches!(
            self,
            Self::ProjectNotFound(_) | Self::InvalidDomain(_) | Self::UpstreamStatus { .. }
        )
    }
}

impl From<reqwest::Error> for CertMonitorError {
    fn from(err: reqwest::Error) -> Self {
        Self::HttpTransport(err.to_string())
    }
}
