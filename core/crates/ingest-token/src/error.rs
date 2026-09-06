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

    /// Prefix is right, length is not. Split out from `WrongPrefix`
    /// because the syntactic check tests two things and used to report
    /// only one of them: `st_bogus` was told to "start with `st_`",
    /// which it does. A reader — human or agent — then goes looking at
    /// the prefix, which is the one part that was correct.
    #[error("token has the right prefix but the wrong length")]
    WrongLength,

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
            Self::WrongLength => {
                "token starts with `st_` but is the wrong length — it was \
                 probably truncated when copied"
            }
            Self::NotFound => "token unknown or revoked",
            Self::KindMismatch { .. } => "token has wrong scope for this endpoint",
            Self::Db(_) => "internal error",
        }
    }
}
