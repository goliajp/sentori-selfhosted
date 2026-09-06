//! Error surface for [`crate::Fingerprint`].

use thiserror::Error;

/// Convenience [`Result`] alias.
pub type FingerprintResult<T> = Result<T, FingerprintError>;

/// All public failure modes the fingerprint API can surface.
///
/// The compute path ([`Fingerprint::compute`]) is total — it always
/// returns a fingerprint. Errors only arise when validating a
/// caller-supplied override via [`Fingerprint::from_override`].
///
/// [`Fingerprint::compute`]: crate::Fingerprint::compute
/// [`Fingerprint::from_override`]: crate::Fingerprint::from_override
#[derive(Debug, Error, PartialEq, Eq)]
pub enum FingerprintError {
    /// Override string was empty.
    ///
    /// Storing an empty fingerprint would collapse every overriding
    /// event into a single nonsense group; reject up front.
    #[error("client override fingerprint must not be empty")]
    OverrideEmpty,

    /// Override string exceeded [`crate::MAX_OVERRIDE_LEN`] bytes.
    ///
    /// Fingerprints land in URLs and DB keys; an unbounded override is
    /// a DoS / index-bloat vector. The cap is generous (256 bytes)
    /// but firm.
    #[error("client override fingerprint too long: {got} bytes, max {max}")]
    OverrideTooLong {
        /// Length the caller submitted, in bytes.
        got: usize,
        /// Configured maximum, equal to [`crate::MAX_OVERRIDE_LEN`].
        max: usize,
    },

    /// Override string contained a control character (`0x00..=0x1F` or
    /// `0x7F`).
    ///
    /// Fingerprints flow through HTTP headers, URLs, log lines, and
    /// DB queries — any of which mishandle control bytes. We reject
    /// rather than silently strip so callers notice the misuse.
    #[error("client override fingerprint contains control character at byte {at}")]
    OverrideControlChar {
        /// Zero-based byte offset of the first offending character.
        at: usize,
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
    fn empty_message_is_clear() {
        let s = FingerprintError::OverrideEmpty.to_string();
        assert!(s.contains("empty"));
    }

    #[test]
    fn too_long_message_includes_sizes() {
        let e = FingerprintError::OverrideTooLong { got: 999, max: 256 };
        let s = e.to_string();
        assert!(s.contains("999"));
        assert!(s.contains("256"));
    }

    #[test]
    fn control_char_message_includes_offset() {
        let s = FingerprintError::OverrideControlChar { at: 7 }.to_string();
        assert!(s.contains('7'.to_string().as_str()));
    }

    #[test]
    fn debug_is_implemented() {
        let _ = format!("{:?}", FingerprintError::OverrideEmpty);
    }

    #[test]
    fn equality_holds_per_variant() {
        assert_eq!(
            FingerprintError::OverrideEmpty,
            FingerprintError::OverrideEmpty
        );
        assert_eq!(
            FingerprintError::OverrideTooLong { got: 1, max: 2 },
            FingerprintError::OverrideTooLong { got: 1, max: 2 }
        );
        assert_ne!(
            FingerprintError::OverrideTooLong { got: 1, max: 2 },
            FingerprintError::OverrideTooLong { got: 1, max: 3 }
        );
    }
}
