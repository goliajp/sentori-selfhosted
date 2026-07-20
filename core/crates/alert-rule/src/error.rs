//! Typed error for the alert-rule crate.

use thiserror::Error;
use uuid::Uuid;

use crate::model::TriggerKindParseError;

/// Failure modes.
#[derive(Debug, Error)]
pub enum AlertRuleError {
    /// Caller-provided draft failed structural validation
    /// (empty name, oversized strings, bad throttle, etc.).
    #[error("invalid rule input: {0}")]
    InvalidInput(String),

    /// Project FK violation on create / update.
    #[error("project {0} not found")]
    ProjectNotFound(Uuid),

    /// Creator FK violation.
    #[error("user {0} not found")]
    UserNotFound(Uuid),

    /// Rule referenced for update / claim / delete wasn't
    /// found.
    #[error("rule {0} not found")]
    RuleNotFound(Uuid),

    /// Stored `trigger_kind` couldn't parse.
    #[error("invalid trigger kind in db: {0}")]
    InvalidTriggerKindInDb(#[from] TriggerKindParseError),

    /// Database error.
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
}

impl AlertRuleError {
    /// True for variants safe to surface to the dashboard.
    #[must_use]
    pub const fn is_safe_for_end_user(&self) -> bool {
        matches!(
            self,
            Self::InvalidInput(_)
                | Self::ProjectNotFound(_)
                | Self::UserNotFound(_)
                | Self::RuleNotFound(_)
        )
    }
}
