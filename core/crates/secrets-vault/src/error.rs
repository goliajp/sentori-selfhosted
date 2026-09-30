//! Error types for the secrets-vault stone.

use core::fmt;

/// Convenience alias for [`crate::Vault::seal`] errors.
pub type SealResult<T> = Result<T, SealError>;

/// Convenience alias for [`crate::Vault::open`] errors.
pub type OpenResult<T> = Result<T, OpenError>;

/// Convenience alias for [`crate::KeyId::new`] errors.
pub type KeyIdResult<T> = Result<T, KeyIdError>;

/// Errors returned by [`crate::Vault::seal`].
#[derive(Debug)]
#[non_exhaustive]
pub enum SealError {
    /// OS CSPRNG (`getrandom`) refused the DEK / nonce request.
    /// Practically never happens on a healthy system.
    EntropyFailure,
    /// AES-GCM `encrypt` returned an error. Sole real-world
    /// trigger: the OS allocator failed inside the cipher's
    /// `Vec` growth, which on Tier-1 targets means the process
    /// is already OOM-aborting. We surface it for completeness
    /// rather than panic.
    EncryptFailed,
}

impl fmt::Display for SealError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EntropyFailure => f.write_str("OS CSPRNG (getrandom) failed"),
            Self::EncryptFailed => f.write_str("AES-256-GCM encrypt returned an error"),
        }
    }
}

impl std::error::Error for SealError {}

/// Errors returned by [`crate::Vault::open`].
#[derive(Debug)]
#[non_exhaustive]
pub enum OpenError {
    /// Sealed blob is shorter than the minimum envelope length
    /// (1 version + 1 key_id_len + 12 wrapped-nonce + 48 wrapped-
    /// dek + 12 payload-nonce + 16 payload-tag = 90 bytes).
    Truncated,
    /// First byte of the envelope is not a recognised version.
    /// Current versions: `0x01` (initial v0.1 format).
    UnsupportedVersion(u8),
    /// The `key_id_len` field claimed a length that overruns the
    /// remaining buffer.
    InvalidKeyIdLength,
    /// The sealed blob's key id is not the one this vault was
    /// constructed with. Call [`crate::peek_key_id`] before
    /// `open` to drive rotation logic.
    KeyIdMismatch {
        /// The id the sealed blob carries.
        sealed_with: String,
        /// The id this vault was constructed with.
        expected: String,
    },
    /// AES-GCM authentication tag on the wrapped DEK did not
    /// verify. The blob was tampered with or the master key is
    /// wrong despite matching the key id (key id collision).
    WrappedDekDecryptFailed,
    /// AES-GCM authentication tag on the payload ciphertext did
    /// not verify. The blob was tampered with after the DEK was
    /// generated (rare — the DEK is per-blob, so tampering one
    /// payload doesn't help an attacker against another).
    PayloadDecryptFailed,
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => f.write_str("sealed envelope is shorter than the minimum length"),
            Self::UnsupportedVersion(v) => {
                write!(f, "unsupported sealed envelope version 0x{v:02x}")
            }
            Self::InvalidKeyIdLength => {
                f.write_str("sealed envelope key_id_len overruns the buffer")
            }
            Self::KeyIdMismatch {
                sealed_with,
                expected,
            } => write!(
                f,
                "sealed blob's key id {sealed_with:?} does not match this vault's id {expected:?}"
            ),
            Self::WrappedDekDecryptFailed => {
                f.write_str("wrapped DEK AES-GCM auth tag did not verify (wrong master / tampered)")
            }
            Self::PayloadDecryptFailed => {
                f.write_str("payload AES-GCM auth tag did not verify (tampered ciphertext)")
            }
        }
    }
}

impl std::error::Error for OpenError {}

/// Errors returned by [`crate::KeyId::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyIdError {
    /// Empty string — key ids must be 1-255 bytes.
    Empty,
    /// Length exceeded the 255-byte ceiling (we serialise the
    /// length as a single byte in the envelope).
    TooLong {
        /// The length we were handed.
        actual: usize,
    },
    /// Key id contains a non-ASCII or non-printable byte. The
    /// id appears in error messages so we restrict the alphabet
    /// to avoid log-line tearing / log injection.
    NonPrintableAscii,
}

impl fmt::Display for KeyIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("KeyId must be 1-255 bytes"),
            Self::TooLong { actual } => {
                write!(f, "KeyId is {actual} bytes, ceiling is 255")
            }
            Self::NonPrintableAscii => {
                f.write_str("KeyId must contain only printable ASCII (0x20-0x7E)")
            }
        }
    }
}

impl std::error::Error for KeyIdError {}
