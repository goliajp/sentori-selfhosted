//! Typed error returned by every fallible call in this crate.

use thiserror::Error;

/// Result alias.
pub type PasswordResult<T> = Result<T, PasswordError>;

/// Failure modes for hash / verify.
///
/// The `InvalidParams` and `MalformedHash` variants are typed
/// separately so a router layer can distinguish "the caller
/// passed bad config" (5xx) from "the stored hash is corrupt"
/// (5xx but log loudly — usually means a writer in a different
/// service produced an unparseable string).
#[derive(Debug, Error)]
pub enum PasswordError {
    /// Password exceeded the implementation's ceiling (see
    /// [`crate::MAX_PASSWORD_BYTES`]). Not an algorithmic
    /// limit — pure `DoS` defence so a million-byte input
    /// doesn't tie up an Argon2 worker for seconds.
    #[error("password exceeds maximum length")]
    TooLong,

    /// [`crate::Params`] are outside the argon2 spec's
    /// acceptable ranges. See [`crate::Params::validate`] for
    /// the bounds.
    #[error("invalid argon2 parameters: {0}")]
    InvalidParams(&'static str),

    /// OS CSPRNG salt generation failed. Extremely rare.
    #[error("entropy source unavailable")]
    EntropyFailure,

    /// Stored hash string is not a valid argon2 PHC-format
    /// value (wrong algorithm tag, malformed b64, etc.). The
    /// argon2 verifier collapses several distinct parse errors
    /// into this one variant deliberately — the caller's
    /// realistic response is "treat as wrong password and log".
    #[error("stored hash is malformed or not argon2")]
    MalformedHash,
}

impl From<password_hash::Error> for PasswordError {
    fn from(_: password_hash::Error) -> Self {
        // All password-hash error variants from the parse/verify
        // path collapse into "stored hash isn't argon2 we recognise".
        Self::MalformedHash
    }
}

impl From<argon2::Error> for PasswordError {
    fn from(err: argon2::Error) -> Self {
        match err {
            argon2::Error::MemoryTooLittle
            | argon2::Error::MemoryTooMuch
            | argon2::Error::TimeTooSmall
            | argon2::Error::ThreadsTooFew
            | argon2::Error::ThreadsTooMany => Self::InvalidParams("argon2 rejected param range"),
            _ => Self::MalformedHash,
        }
    }
}
