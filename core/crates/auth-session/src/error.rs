//! Typed error returned by [`crate::AuthService`].

use sentori_argon2_password::PasswordError;
use sentori_workspace_identity::{IdentityError, UserId};
use thiserror::Error;
use uuid::Uuid;

/// Failure modes for the auth-session crate.
///
/// Variants split into three groups:
///
/// 1. **Domain outcomes** the caller renders to end users
///    (`InvalidCredentials`, `EmailNotVerified`, `TokenInvalid`,
///    etc.). [`AuthError::is_safe_for_end_user`] helps router
///    layers decide what's safe to surface verbatim vs collapse
///    to a 500.
/// 2. **Misuse / invariant** — caller passed something the
///    typed surface couldn't catch (e.g. `TokenInvalid` collapses
///    bad format / expired / used so brute-forcers can't
///    distinguish).
/// 3. **Infrastructure** — propagated from sqlx / underlying
///    stones.
#[derive(Debug, Error)]
pub enum AuthError {
    /// Email / password pair didn't match (or user doesn't
    /// exist). Single variant deliberately — leaking "no such
    /// user" from "wrong password" enables user enumeration.
    #[error("invalid email or password")]
    InvalidCredentials,

    /// Login attempted before email verification finished.
    #[error("email is not verified")]
    EmailNotVerified,

    /// Token unknown / malformed / expired / already used.
    /// Collapsed deliberately — see
    /// [`sentori_workspace_identity::IdentityError::InviteInvalid`].
    #[error("token invalid, expired, or already used")]
    TokenInvalid,

    /// Password failed structural validation (too short, etc.).
    #[error("password too short (minimum {min} chars, got {got})")]
    PasswordTooShort {
        /// Configured minimum length.
        min: usize,
        /// Provided length.
        got: usize,
    },

    /// Email failed structural validation.
    #[error("email is not a plausible address")]
    EmailInvalid,

    /// `change_password` called with a wrong `current_password`.
    /// Distinct from `InvalidCredentials` because the caller is
    /// already authenticated — surfacing "wrong current pwd" is
    /// fine here and matches Sentori's existing dashboard copy.
    #[error("current password is incorrect")]
    CurrentPasswordWrong,

    /// Session id was found in DB but had no matching user (FK
    /// dangling). Treated as unauthorized at the router; logged
    /// loudly because it should never happen.
    #[error("session {0} references a missing user")]
    SessionDangling(Uuid),

    /// Tried to find a session that doesn't exist or is expired.
    #[error("no active session")]
    NoActiveSession,

    /// User id passed to a high-level method doesn't exist.
    #[error("user {0} not found")]
    UserNotFound(UserId),

    /// Underlying [`sentori_workspace_identity`] error.
    #[error(transparent)]
    Identity(#[from] IdentityError),

    /// Underlying [`sentori_argon2_password`] error.
    #[error(transparent)]
    Password(#[from] PasswordError),

    /// Underlying cookie signing / parsing failure.
    #[error("cookie verification failed")]
    CookieInvalid,

    /// CSPRNG failed (extremely rare).
    #[error("entropy source unavailable: {0}")]
    Entropy(String),

    /// Database error.
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
}

impl AuthError {
    /// True if the variant is safe to render verbatim to the end
    /// user. False for infra / invariant variants.
    #[must_use]
    pub const fn is_safe_for_end_user(&self) -> bool {
        matches!(
            self,
            Self::InvalidCredentials
                | Self::EmailNotVerified
                | Self::TokenInvalid
                | Self::PasswordTooShort { .. }
                | Self::EmailInvalid
                | Self::CurrentPasswordWrong
                | Self::NoActiveSession
        )
    }
}
