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

    /// True when trying again can never change the outcome.
    ///
    /// A caller draining a queue has to tell these apart. Treating a
    /// permanent failure as transient is not a slow retry, it is an
    /// infinite one: the row never leaves the queue, the same error
    /// repeats forever, and the log fills with a warning nobody can
    /// act on because it looks like a blip.
    ///
    /// A foreign-key violation is the case that matters here. It means
    /// the row points at something that does not exist — a workspace
    /// deleted while its Stripe subscription was still live, say — and
    /// no amount of waiting will bring it back.
    #[must_use]
    pub fn is_permanent(&self) -> bool {
        match self {
            // Bad input, a missing row, a tag the database holds that
            // this build cannot parse: all of these are the same
            // tomorrow.
            Self::InvalidInput(_)
            | Self::ProjectNotFound(_)
            | Self::AlreadyInitialised
            | Self::NotInitialised
            | Self::InvalidPlanInDb(_)
            | Self::InvalidStatusInDb(_)
            | Self::InvalidCounterKindInDb(_) => true,
            Self::Db(e) => e
                .as_database_error()
                .and_then(sqlx::error::DatabaseError::code)
                .is_some_and(|c| sqlstate_is_permanent(&c)),
        }
    }
}

/// Whether a Postgres SQLSTATE describes a failure that will recur.
///
/// Class 23 is integrity constraint violation — foreign key, unique,
/// not-null, check. The data is wrong, not the moment. Everything else
/// (connection loss, serialization failure, deadlock, lock timeout) is
/// worth another attempt.
fn sqlstate_is_permanent(code: &str) -> bool {
    code.starts_with("23")
}

#[cfg(test)]
mod permanence_tests {
    use super::*;

    /// The classification is what stands between a queue and an
    /// infinite loop, so the boundary is pinned rather than left to
    /// whoever next edits the match arm.
    #[test]
    fn constraint_violations_are_permanent_transport_faults_are_not() {
        // 23xxx — the data is wrong.
        assert!(sqlstate_is_permanent("23503")); // foreign_key_violation
        assert!(sqlstate_is_permanent("23505")); // unique_violation
        assert!(sqlstate_is_permanent("23502")); // not_null_violation
        assert!(sqlstate_is_permanent("23514")); // check_violation

        // The moment is wrong; try again.
        assert!(!sqlstate_is_permanent("40001")); // serialization_failure
        assert!(!sqlstate_is_permanent("40P01")); // deadlock_detected
        assert!(!sqlstate_is_permanent("55P03")); // lock_not_available
        assert!(!sqlstate_is_permanent("08006")); // connection_failure
        assert!(!sqlstate_is_permanent("53300")); // too_many_connections
        assert!(!sqlstate_is_permanent("57014")); // query_canceled
    }

    #[test]
    fn non_database_variants_are_permanent() {
        assert!(BillingError::NotInitialised.is_permanent());
        assert!(BillingError::AlreadyInitialised.is_permanent());
        assert!(BillingError::InvalidInput("x".into()).is_permanent());
        assert!(BillingError::ProjectNotFound(Uuid::nil()).is_permanent());
    }

    /// A pool timeout carries no SQLSTATE. It has to fall through to
    /// transient — the opposite would drop real events on a blip.
    #[test]
    fn errors_without_a_sqlstate_are_transient() {
        assert!(!BillingError::Db(sqlx::Error::PoolTimedOut).is_permanent());
        assert!(!BillingError::Db(sqlx::Error::RowNotFound).is_permanent());
    }
}
