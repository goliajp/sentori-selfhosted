//! Typed error for the runtime-metrics crate.

use thiserror::Error;
use uuid::Uuid;

use crate::model::DropReasonParseError;

/// Failure modes for the runtime-metrics surface.
#[derive(Debug, Error)]
pub enum RuntimeMetricsError {
    /// Project FK violation on ingest.
    #[error("project {0} not found")]
    ProjectNotFound(Uuid),

    /// Structural validation failure on input.
    #[error("metric input invalid: {0}")]
    InvalidInput(String),

    /// Partition operation referenced an unparseable child
    /// table name. Should be unreachable.
    #[error("invalid partition name: {0:?}")]
    InvalidPartitionName(String),

    /// Stored `runtime_metrics_dropped.reason` outside the
    /// canonical set.
    #[error("invalid drop reason in database: {0}")]
    InvalidDropReasonInDb(#[from] DropReasonParseError),

    /// Database error.
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
}

impl RuntimeMetricsError {
    /// True for variants safe to surface verbatim to the SDK.
    #[must_use]
    pub const fn is_safe_for_end_user(&self) -> bool {
        matches!(self, Self::ProjectNotFound(_) | Self::InvalidInput(_))
    }
}
