//! Typed errors for [`crate::BillingService`].

use thiserror::Error;
use uuid::Uuid;

use crate::model::{CounterKindParseError, PlanParseError, PlanStatusParseError};

/// Failure modes.
#[derive(Debug, Error)]
pub enum BillingError {
    /// Caller passed bad input (negative delta, etc.).
    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// Project FK violation on counter update.
    #[error("project {0} not found")]
    ProjectNotFound(Uuid),

    /// `workspace_billing` is a DB-enforced singleton; if
    /// caller tries to insert a second row, this fires.
    #[error("workspace_billing singleton row already exists")]
    AlreadyInitialised,

    /// `get_plan` / mutation paths that need the singleton
    /// to exist found no row.
    #[error("workspace_billing not initialised — call ensure_default first")]
    NotInitialised,

    /// Persisted plan tag couldn't parse.
    #[error("invalid plan in db: {0}")]
    InvalidPlanInDb(#[from] PlanParseError),

    /// Persisted status tag couldn't parse.
    #[error("invalid status in db: {0}")]
    InvalidStatusInDb(#[from] PlanStatusParseError),

    /// Persisted counter_kind tag couldn't parse.
    #[error("invalid counter_kind in db: {0}")]
    InvalidCounterKindInDb(#[from] CounterKindParseError),

    /// Database error.
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
}

impl BillingError {
    /// True for variants safe to surface verbatim to the
    /// dashboard.
    #[must_use]
    pub const fn is_safe_for_end_user(&self) -> bool {
        matches!(
            self,
            Self::InvalidInput(_)
                | Self::ProjectNotFound(_)
                | Self::AlreadyInitialised
                | Self::NotInitialised
        )
    }
}
