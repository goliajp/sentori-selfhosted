//! Error surface for [`crate::Hasher`].

use thiserror::Error;

/// Convenience [`Result`] alias.
pub type PrivacySaltResult<T> = Result<T, PrivacySaltError>;

/// All public failure modes the hasher can surface.
#[derive(Debug, Error)]
pub enum PrivacySaltError {
    /// Master secret shorter than [`crate::MIN_MASTER_SECRET_BYTES`].
    ///
    /// Sentori requires at least 32 bytes of entropy in the master
    /// secret; accepting a shorter key would silently weaken the entire
    /// PII-hash pipeline. This is the only error path [`Hasher::new`]
    /// can return.
    ///
    /// [`Hasher::new`]: crate::Hasher::new
    #[error("master secret too short: {got} bytes, need >= {min}")]
    MasterSecretTooShort {
        /// Number of bytes the caller supplied.
        got: usize,
        /// Required minimum, set by [`crate::MIN_MASTER_SECRET_BYTES`].
        min: usize,
    },
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::missing_panics_doc
)]
mod tests {
    use super::*;

    #[test]
    fn display_message_includes_sizes() {
        let e = PrivacySaltError::MasterSecretTooShort { got: 10, min: 32 };
        let s = e.to_string();
        assert!(s.contains("10"));
        assert!(s.contains("32"));
        assert!(s.contains("master secret"));
    }

    #[test]
    fn debug_is_implemented() {
        let e = PrivacySaltError::MasterSecretTooShort { got: 0, min: 32 };
        let _ = format!("{e:?}");
    }
}
