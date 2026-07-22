//! Typed error returned by every [`crate::IssueStore`] method.

use sentori_event_pipeline::{EventKindParseError, IssueStatusParseError};
use thiserror::Error;
use uuid::Uuid;

use crate::cursor::CursorParseError;

/// Failure modes for the issue-store.
#[derive(Debug, Error)]
pub enum IssueStoreError {
    /// Cursor string was malformed (wrong length, bad base64,
    /// or fields outside their valid ranges).
    #[error("invalid cursor: {0}")]
    InvalidCursor(#[from] CursorParseError),

    /// `merge(src, dst)` was called with `src == dst` — a no-op
    /// we surface so the operator UI's "merge into …" picker
    /// can't accidentally produce data loss.
    #[error("cannot merge an issue into itself")]
    MergeIntoSelf,

    /// `merge(src, dst)` was called across project boundaries.
    /// We reject — merging cross-project issues would mix
    /// fingerprints from different privacy-salt scopes and
    /// break D6 dump correctness.
    #[error("cannot merge issues across projects")]
    MergeAcrossProjects,

    /// `merge(src, dst)` or `patch(issue_id, …)` referenced an
    /// issue that doesn't exist.
    #[error("issue {0} not found")]
    IssueNotFound(Uuid),

    /// `IssuePatch::priority` was not one of `p0`/`p1`/`p2`/`p3`.
    #[error("invalid priority {got:?}, must be one of p0/p1/p2/p3")]
    InvalidPriority {
        /// What the caller passed.
        got: String,
    },

    /// `IssuePatch::status` was not one of the four canonical
    /// values.
    #[error("invalid status: {0}")]
    InvalidStatus(#[from] IssueStatusParseError),

    /// Stored issue row had a corrupt enum string. Should be
    /// unreachable; surface loudly.
    #[error("invalid event kind in database: {0}")]
    InvalidKindInDb(#[from] EventKindParseError),

    /// Database error.
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
}

impl IssueStoreError {
    /// True for variants safe to surface to the dashboard
    /// verbatim. False for infra failures (DB).
    #[must_use]
    pub const fn is_safe_for_end_user(&self) -> bool {
        matches!(
            self,
            Self::InvalidCursor(_)
                | Self::MergeIntoSelf
                | Self::MergeAcrossProjects
                | Self::IssueNotFound(_)
                | Self::InvalidPriority { .. }
                | Self::InvalidStatus(_)
        )
    }
}
