//! TokenError — all failure modes of token auth.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum TokenError {
    #[error("missing Authorization header")]
    MissingHeader,

    #[error("Authorization header malformed (expected 'Bearer st_pk_...')")]
    MalformedHeader,

    #[error("token must start with `st_pk_`")]
    WrongPrefix,

    #[error("token not found or revoked")]
    NotFound,

    #[error("token kind mismatch (got {got:?}, expected {expected:?})")]
    KindMismatch {
        got: crate::TokenKind,
        expected: crate::TokenKind,
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
            Self::MissingHeader => "send `Authorization: Bearer st_pk_<token>` header",
            Self::MalformedHeader => "Authorization header must be `Bearer st_pk_<token>`",
            Self::WrongPrefix => "token must start with `st_pk_`",
            Self::NotFound => "token unknown or revoked",
            Self::KindMismatch { .. } => "token has wrong kind for this endpoint",
            Self::Db(_) => "internal error",
        }
    }
}
