//! TokenError — all failure modes of token auth.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum TokenError {
    #[error("missing Authorization header")]
    MissingHeader,

    #[error("Authorization header malformed (expected 'Bearer st_...')")]
    MalformedHeader,

    #[error("token must start with `st_`")]
    WrongPrefix,

    #[error("token not found or revoked")]
    NotFound,

    #[error("token scope mismatch (got {got:?}, expected {expected:?})")]
    KindMismatch {
        got: crate::Scope,
        expected: crate::Scope,
    },

    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
}

impl TokenError {
    /// User-safe hint string for 401 responses. Always
    /// disambiguates the failure mode WITHOUT leaking whether
    /// the token exists.
    #[must_use]
    pub fn user_hint(&self) -> &'static str {
        match self {
            Self::MissingHeader => "send `Authorization: Bearer st_<token>` header",
            Self::MalformedHeader => "Authorization header must be `Bearer st_<token>`",
            Self::WrongPrefix => "token must start with `st_`",
            Self::NotFound => "token unknown or revoked",
            Self::KindMismatch { .. } => "token has wrong scope for this endpoint",
            Self::Db(_) => "internal error",
        }
    }
}
