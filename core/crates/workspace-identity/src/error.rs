//! Typed error returned by every fallible call in this crate.

use thiserror::Error;
use uuid::Uuid;

use crate::model::{RoleParseError, UserId};

/// Error returned by [`crate::Identity`] sub-handles.
///
/// Variants split into three groups:
///
/// 1. **Not-found / preconditions** — domain-level outcomes the
///    caller is expected to render to the end user (e.g. show a
///    "user already exists" page).
/// 2. **Invariant violations** — bugs in the caller's flow.
///    Should never reach the end user as-is; surface as a 500.
///    Examples: trying to grant project visibility to an
///    admin (admins auto-see everything; granting is a no-op
///    bug at best), or invite-accept attempting to satisfy the
///    DB-level "one owner" partial unique index without first
///    demoting the previous owner.
/// 3. **Infrastructure** — propagated from sqlx. The caller
///    layer (K2) decides whether to retry or surface.
#[derive(Debug, Error)]
pub enum IdentityError {
    /// A user with the given id does not exist.
    #[error("user {0} not found")]
    UserNotFound(UserId),

    /// A user with the requested email is already on file.
    /// Case-insensitive — `Alice@x.com` collides with `alice@x.com`.
    #[error("email already registered")]
    EmailTaken,

    /// A project with the requested slug is already on file.
    #[error("project slug already taken: {0}")]
    SlugTaken(String),

    /// A project with the given id does not exist.
    #[error("project {0} not found")]
    ProjectNotFound(Uuid),

    /// `workspace_members` row not found for this user.
    #[error("user {0} is not a workspace member")]
    NotAMember(UserId),

    /// Tried to grant per-project visibility to a user whose role
    /// is owner or admin — those roles auto-see every project,
    /// so an explicit grant is a logic error.
    #[error("cannot grant visibility to owner/admin (they see all projects)")]
    VisibilityRefusedForElevatedRole,

    /// The invite token did not match any pending invite, the
    /// invite has expired, or the invite has already been
    /// accepted. The three are deliberately collapsed so we do
    /// not leak the existence of stale/accepted invites to
    /// brute-forcing attackers.
    #[error("invite token invalid, expired, or already used")]
    InviteInvalid,

    /// Tried to mint an invite with `expires_in_days == 0` or
    /// greater than [`crate::Invites::MAX_EXPIRES_IN_DAYS`].
    #[error("invite expiry must be between 1 and {max} days, got {got}")]
    InviteExpiryOutOfRange {
        /// The rejected value, in days.
        got: i64,
        /// The crate-enforced upper bound, in days.
        max: i64,
    },

    /// Tried to transfer owner to a user who is not currently a
    /// workspace member. The caller must add the user first
    /// (typically as admin) and then transfer.
    #[error("transfer target {0} is not a workspace member")]
    TransferTargetNotMember(UserId),

    /// Tried to transfer owner to the current owner — a no-op
    /// that we surface as an error to make the caller's intent
    /// explicit (avoids accidental "transfer to self" double-
    /// click bugs).
    #[error("transfer target is already the owner")]
    TransferTargetAlreadyOwner,

    /// Internal: a database row stored a `role` string outside
    /// `('owner','admin','user')`. The DB CHECK constraint
    /// should make this impossible — surfacing it means the
    /// schema drifted or the row was inserted around our types.
    #[error("invalid role stored in database: {0}")]
    InvalidRoleInDatabase(#[from] RoleParseError),

    /// Database operation failed.
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),

    /// CSPRNG failed (extremely rare; only on broken systems).
    #[error("entropy source unavailable: {0}")]
    Entropy(String),
}

impl IdentityError {
    /// Returns `true` if this error is safe to render verbatim to
    /// an end user. False for infra / invariant variants that
    /// would leak implementation detail.
    #[must_use]
    pub const fn is_safe_for_end_user(&self) -> bool {
        matches!(
            self,
            Self::UserNotFound(_)
                | Self::EmailTaken
                | Self::SlugTaken(_)
                | Self::ProjectNotFound(_)
                | Self::NotAMember(_)
                | Self::InviteInvalid
                | Self::InviteExpiryOutOfRange { .. }
                | Self::TransferTargetNotMember(_)
                | Self::TransferTargetAlreadyOwner
        )
    }
}
