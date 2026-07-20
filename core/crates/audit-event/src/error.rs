//! Typed error for [`crate::AuditService`].

use thiserror::Error;
use uuid::Uuid;

/// Failure modes for the audit service.
#[derive(Debug, Error)]
pub enum AuditError {
    /// Draft failed structural validation (empty / oversize
    /// action, etc.).
    #[error("invalid audit entry: {0}")]
    InvalidInput(String),

    /// Project FK violation on record.
    #[error("project {0} not found")]
    ProjectNotFound(Uuid),

    /// Actor FK violation on record.
    #[error("actor user {0} not found")]
    ActorNotFound(Uuid),

    /// Database error.
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
}

impl AuditError {
    /// True for variants safe to surface verbatim to the
    /// dashboard.
    #[must_use]
    pub const fn is_safe_for_end_user(&self) -> bool {
        matches!(
            self,
            Self::InvalidInput(_) | Self::ProjectNotFound(_) | Self::ActorNotFound(_)
        )
    }
}
