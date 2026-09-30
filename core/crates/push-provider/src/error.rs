//! Typed error returned by every [`crate::PushDispatcher`] /
//! [`crate::DeviceTokenStore`] / [`crate::CredentialStore`]
//! call.

use sentori_secrets_vault::OpenError;
use thiserror::Error;
use uuid::Uuid;

use crate::model::ProviderKindParseError;

/// Failure modes for the push-provider crate.
#[derive(Debug, Error)]
pub enum PushError {
    /// Project referenced doesn't exist (FK violation).
    #[error("project {0} not found")]
    ProjectNotFound(Uuid),

    /// No `push_credentials` row for `(project, provider)`.
    /// Caller must store credentials before dispatch.
    #[error("no credentials configured for project {project_id} / {kind:?}")]
    CredentialsMissing {
        /// Project that's missing the credentials.
        project_id: Uuid,
        /// Which provider.
        kind: String,
    },

    /// No provider impl registered for the requested kind.
    /// Common during K7's iterative ship — only registered
    /// providers can dispatch.
    #[error("no provider registered for kind {0:?}")]
    ProviderNotRegistered(String),

    /// `dispatch_target = SingleToken { token_id }` with no
    /// matching row, OR the row exists but is quarantined.
    #[error("token {0} not found or quarantined")]
    TokenNotFound(Uuid),

    /// The S10 rate-limiter rejected this dispatch (the
    /// per-(project, provider) L1 hit its window cap).
    #[error("rate-limited (retry after ~{retry_after_ms} ms)")]
    RateLimited {
        /// Approximate ms until the window has capacity again.
        retry_after_ms: i64,
    },

    /// Input validation failed (empty title, malformed
    /// custom data, etc.).
    #[error("push input invalid: {0}")]
    InvalidInput(String),

    /// S12 vault unseal failed when loading credentials.
    /// Means the credential row was corrupted or sealed with
    /// a different master key.
    #[error("credential decryption failed: {0}")]
    CredentialUnseal(#[from] OpenError),

    /// Stored row had a corrupt enum string (kind /
    /// validate-status). Should be unreachable.
    #[error("invalid provider kind in database: {0}")]
    InvalidKindInDb(#[from] ProviderKindParseError),

    /// Provider returned a typed error (auth failure, network
    /// transport, etc.). Caller can branch on the inner
    /// message but most go to logs.
    #[error("provider {kind:?} error: {message}")]
    Provider {
        /// Which provider raised the error.
        kind: String,
        /// Provider-supplied human-readable detail.
        message: String,
    },

    /// Database error.
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
}

impl PushError {
    /// True if the variant is safe to render to the SDK / end
    /// user verbatim.
    #[must_use]
    pub const fn is_safe_for_end_user(&self) -> bool {
        matches!(
            self,
            Self::ProjectNotFound(_)
                | Self::CredentialsMissing { .. }
                | Self::ProviderNotRegistered(_)
                | Self::TokenNotFound(_)
                | Self::RateLimited { .. }
                | Self::InvalidInput(_)
        )
    }
}
