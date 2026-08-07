//! Typed error for [`crate::ReplayStore`].

use sentori_attachment_store::BlobError;
use sentori_issue_store::CursorParseError;
use thiserror::Error;
use uuid::Uuid;

use crate::scrubber::ScrubberError;

/// Failure modes for the replay store.
#[derive(Debug, Error)]
pub enum ReplayStoreError {
    /// Project FK violation.
    #[error("project {0} not found")]
    ProjectNotFound(Uuid),

    /// Event FK violation — `store(event_id, …)` was called
    /// with an id that doesn't match any row in `events`.
    /// Common during testing where the test forgot to seed
    /// the event before the replay.
    #[error("event {0} not found")]
    EventNotFound(Uuid),

    /// `fetch` / `find` / `delete` referenced a session that
    /// doesn't exist.
    #[error("replay session {0} not found")]
    SessionNotFound(Uuid),

    /// `started_at > ended_at` or `frame_count < 0` — caller
    /// supplied an inconsistent input.
    #[error("replay input invalid: {0}")]
    InvalidInput(String),

    /// Cursor string was malformed.
    #[error("invalid cursor: {0}")]
    InvalidCursor(#[from] CursorParseError),

    /// The scrubber pipeline failed (regex / encoding).
    #[error("scrub failed: {0}")]
    Scrub(#[from] ScrubberError),

    /// K3 attachment-store backend returned an error.
    #[error("blob store error: {0}")]
    Blob(#[from] BlobError),

    /// Database error.
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),

    /// gzip / gunzip failure.
    #[error("compression error: {0}")]
    Compression(String),
}

impl ReplayStoreError {
    /// True for variants safe to surface verbatim to the
    /// dashboard. False for infra / invariant variants.
    #[must_use]
    pub const fn is_safe_for_end_user(&self) -> bool {
        matches!(
            self,
            Self::ProjectNotFound(_)
                | Self::EventNotFound(_)
                | Self::SessionNotFound(_)
                | Self::InvalidInput(_)
                | Self::InvalidCursor(_)
        )
    }
}

impl From<std::io::Error> for ReplayStoreError {
    fn from(err: std::io::Error) -> Self {
        Self::Compression(err.to_string())
    }
}
