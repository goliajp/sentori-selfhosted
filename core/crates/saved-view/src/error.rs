//! Typed errors for [`crate::SavedViewService`].

use thiserror::Error;
use uuid::Uuid;

use crate::model::{ScopeParseError, TargetParseError};

/// Failure modes.
#[derive(Debug, Error)]
pub enum SavedViewError {
    /// Caller-provided draft / patch failed structural
    /// validation (empty name, oversize, scope-FK polarity
    /// violation, etc.).
    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// Project FK violation on create.
    #[error("project {0} not found")]
    ProjectNotFound(Uuid),

    /// Owning user (personal scope) or creator FK violation.
    #[error("user {0} not found")]
    UserNotFound(Uuid),

    /// View referenced for update / delete wasn't found.
    #[error("view {0} not found")]
    ViewNotFound(Uuid),

    /// Stored target couldn't parse.
    #[error("invalid target in db: {0}")]
    InvalidTargetInDb(#[from] TargetParseError),

    /// Stored scope couldn't parse.
    #[error("invalid scope in db: {0}")]
    InvalidScopeInDb(#[from] ScopeParseError),

    /// Database error.
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
}

impl SavedViewError {
    /// True for variants safe to surface verbatim to the
    /// dashboard.
    #[must_use]
    pub const fn is_safe_for_end_user(&self) -> bool {
        matches!(
            self,
            Self::InvalidInput(_)
                | Self::ProjectNotFound(_)
                | Self::UserNotFound(_)
                | Self::ViewNotFound(_)
        )
    }
}
