//! Typed error returned by [`crate::IngestService`].

use thiserror::Error;
use uuid::Uuid;

use crate::model::{
    EventKindParseError, IssueStatusParseError, MessageLevelParseError, PlatformParseError,
};

/// Failure modes for the ingest pipeline.
///
/// Splits into three groups: validation failures the caller can
/// render to the SDK as 4xx, infrastructure failures (DB), and
/// internal invariant breaks (corrupt rows).
#[derive(Debug, Error)]
pub enum IngestError {
    /// Event failed structural validation.
    #[error("event invalid: {0}")]
    InvalidEvent(String),

    /// Ring buffer rejected the push under concurrent contention
    /// (the bounded ring's lose-the-race path). The caller can
    /// retry or treat as drop; we surface it so caller telemetry
    /// matches what S4's `dropped_count` records.
    #[error("ring buffer full, event dropped")]
    RingDropped,

    /// The project referenced by `ingest(project_id, …)` was not
    /// found (FK violation on issues / events insert).
    #[error("project {0} not found")]
    ProjectNotFound(Uuid),

    /// Stored issue row had a corrupt `status` string (outside
    /// the CHECK constraint set). Should be unreachable in normal
    /// operation; surface loudly if it happens.
    #[error("invalid issue status in database: {0}")]
    InvalidIssueStatusInDb(#[from] IssueStatusParseError),

    /// Stored event row had a corrupt `kind` string.
    #[error("invalid event kind in database: {0}")]
    InvalidEventKindInDb(#[from] EventKindParseError),

    /// Stored event row had a corrupt `platform` string.
    #[error("invalid platform in database: {0}")]
    InvalidPlatformInDb(#[from] PlatformParseError),

    /// Stored event payload referenced a corrupt message level.
    #[error("invalid message level in database: {0}")]
    InvalidMessageLevelInDb(#[from] MessageLevelParseError),

    /// Database error.
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
}

impl IngestError {
    /// True for variants the consumer can surface to the SDK as
    /// a structured 4xx. False for infra / invariant — those go
    /// to logs.
    #[must_use]
    pub const fn is_safe_for_sdk(&self) -> bool {
        matches!(
            self,
            Self::InvalidEvent(_) | Self::RingDropped | Self::ProjectNotFound(_)
        )
    }
}
