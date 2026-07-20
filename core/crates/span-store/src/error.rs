//! Typed error returned by [`crate::SpanStore`] methods.

use sentori_issue_store::CursorParseError;
use thiserror::Error;
use uuid::Uuid;

use crate::model::SpanStatusParseError;

/// Failure modes for the span-store.
#[derive(Debug, Error)]
pub enum SpanStoreError {
    /// Span input failed structural validation (duration < 0,
    /// op too long, etc.).
    #[error("span invalid: {0}")]
    InvalidSpan(String),

    /// Cursor string was malformed.
    #[error("invalid cursor: {0}")]
    InvalidCursor(#[from] CursorParseError),

    /// The project referenced by `ingest_span(project_id, …)`
    /// was not found (FK violation on spans + traces insert).
    #[error("project {0} not found")]
    ProjectNotFound(Uuid),

    /// Trace lookup target not found.
    #[error("trace {0} not found")]
    TraceNotFound(Uuid),

    /// Stored span / trace row had a corrupt `status` string.
    /// Should be unreachable in normal operation.
    #[error("invalid status in database: {0}")]
    InvalidStatusInDb(#[from] SpanStatusParseError),

    /// A partition operation referenced a name outside the
    /// expected `<table>_YYYY_MM` shape. Should be unreachable;
    /// the lifecycle builds names itself.
    #[error("invalid partition name: {0:?}")]
    InvalidPartitionName(String),

    /// Database error.
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
}

impl SpanStoreError {
    /// True for variants safe to surface to the SDK / dashboard.
    /// False for infra (DB) / invariant (corrupt status).
    #[must_use]
    pub const fn is_safe_for_end_user(&self) -> bool {
        matches!(
            self,
            Self::InvalidSpan(_)
                | Self::InvalidCursor(_)
                | Self::ProjectNotFound(_)
                | Self::TraceNotFound(_)
        )
    }
}
