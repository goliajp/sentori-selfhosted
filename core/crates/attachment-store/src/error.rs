//! Typed error returned by every [`crate::BlobStore`] method.

use thiserror::Error;

use crate::hash::BlobHashParseError;

/// Result alias.
pub type BlobResult<T> = Result<T, BlobError>;

/// Blob-store failure modes.
///
/// The four variants split into "domain" (not-found, hash
/// mismatch, malformed key) and "infrastructure" (backend I/O).
#[derive(Debug, Error)]
pub enum BlobError {
    /// No blob with the requested hash exists.
    #[error("blob not found")]
    NotFound,

    /// `get_verified` fetched a blob whose actual SHA-256 did
    /// not match the requested hash. Indicates backend corruption
    /// or tampering — surface loudly; never silently retry.
    #[error("blob hash mismatch (backend returned corrupt or swapped bytes)")]
    HashMismatch,

    /// Caller passed a string that wasn't a valid [`crate::BlobHash`]
    /// (wrong length, non-hex characters, …).
    #[error("invalid blob hash: {0}")]
    InvalidHash(#[from] BlobHashParseError),

    /// Underlying filesystem / network / API error from the
    /// backing store. Carries a descriptive message; the
    /// originating error chain is preserved in the source.
    #[error("backend error: {message}")]
    Backend {
        /// Human-readable explanation.
        message: String,
        /// Original error, if any.
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync + 'static>>,
    },
}

impl BlobError {
    /// Construct a [`BlobError::Backend`] from an arbitrary
    /// error type. Useful for backends that wrap `std::io::Error`
    /// or an SDK error type.
    pub fn backend<E>(message: impl Into<String>, source: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync + 'static>>,
    {
        Self::Backend {
            message: message.into(),
            source: Some(source.into()),
        }
    }

    /// Construct a [`BlobError::Backend`] with no source error
    /// (when the failure is a context the backend can't carry).
    pub fn backend_msg(message: impl Into<String>) -> Self {
        Self::Backend {
            message: message.into(),
            source: None,
        }
    }

    /// True for variants the caller can render to end users
    /// (`NotFound`, `InvalidHash`, `HashMismatch`). False for
    /// raw backend errors — those go to logs, not to the user.
    #[must_use]
    pub const fn is_safe_for_end_user(&self) -> bool {
        matches!(
            self,
            Self::NotFound | Self::HashMismatch | Self::InvalidHash(_)
        )
    }
}

impl From<std::io::Error> for BlobError {
    fn from(err: std::io::Error) -> Self {
        if err.kind() == std::io::ErrorKind::NotFound {
            Self::NotFound
        } else {
            Self::backend(format!("io: {err}"), err)
        }
    }
}
