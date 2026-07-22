//! Error types for the cookie-session stone.
//!
//! Distinct types per primitive so callers can branch cleanly
//! without losing the upstream error context.

use core::fmt;

/// Convenience alias for [`SignedCookie`](crate::SignedCookie)
/// verify errors.
pub type SignedCookieResult<T> = Result<T, SignedCookieError>;

/// Convenience alias for [`EncryptedCookie`](crate::EncryptedCookie)
/// open errors.
pub type EncryptedCookieResult<T> = Result<T, EncryptedCookieError>;

/// Convenience alias for [`PasswordHash`](crate::PasswordHash)
/// errors.
pub type PasswordResult<T> = Result<T, PasswordError>;

/// Convenience alias for [`CsrfToken`](crate::CsrfToken) errors.
pub type CsrfResult<T> = Result<T, CsrfError>;

// ── SignedCookie ─────────────────────────────────────────────

/// Errors returned by [`crate::SignedCookie::open`].
///
/// `MalformedEncoding` and `BadSignature` are deliberately distinct
/// so an operator monitoring the dashboard can tell "garbage value"
/// (typically: pre-deploy cookie, third-party tampering, cookie
/// truncation in transit) from "wrong key" (typically: secret
/// rotation, accidental key swap between services).
#[derive(Debug)]
#[non_exhaustive]
pub enum SignedCookieError {
    /// The encoded cookie value is not valid base64-url-no-pad, or
    /// is shorter than the minimum (32-byte tag) length.
    MalformedEncoding,
    /// The signature did not verify against the supplied key — the
    /// payload was tampered with or signed by a different key.
    BadSignature,
}

impl fmt::Display for SignedCookieError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedEncoding => {
                f.write_str("signed cookie value is not valid base64-url-no-pad")
            }
            Self::BadSignature => f.write_str("signed cookie HMAC signature did not verify"),
        }
    }
}

impl std::error::Error for SignedCookieError {}

// ── EncryptedCookie ─────────────────────────────────────────

/// Errors returned by [`crate::EncryptedCookie::open`].
#[derive(Debug)]
#[non_exhaustive]
pub enum EncryptedCookieError {
    /// The encoded cookie value is not valid base64-url-no-pad, or
    /// is shorter than the minimum (12-byte nonce + 16-byte tag).
    MalformedEncoding,
    /// AES-GCM authentication tag did not verify — the ciphertext
    /// was tampered with, the nonce was reused, or the key is
    /// wrong.
    Decrypt,
}

impl fmt::Display for EncryptedCookieError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedEncoding => {
                f.write_str("encrypted cookie value is not valid base64-url-no-pad")
            }
            Self::Decrypt => f.write_str("encrypted cookie AEAD decrypt failed"),
        }
    }
}

impl std::error::Error for EncryptedCookieError {}

// ── PasswordHash ─────────────────────────────────────────────

/// Errors returned by [`crate::PasswordHash::hash`] /
/// [`crate::PasswordHash::verify`].
#[derive(Debug)]
#[non_exhaustive]
pub enum PasswordError {
    /// Password exceeded bcrypt's 72-byte input limit. The legacy
    /// behaviour is silent truncation, which we refuse: callers
    /// must explicitly hash-then-bcrypt (typically SHA-256-first)
    /// if they want to accept longer passwords.
    TooLong,
    /// The configured cost is outside the bcrypt-valid range
    /// (4-31). Callers default to [`crate::PasswordHash::COST_DEFAULT`]
    /// which is always valid.
    InvalidCost,
    /// The stored hash string is not a valid bcrypt(5) modular
    /// crypt format string (`$2[abxy]$cost$22-char-salt+31-char-hash`).
    MalformedHash,
    /// CSPRNG salt generation failed (`getrandom` returned an
    /// error). Practically never happens on a healthy OS.
    EntropyFailure,
}

impl fmt::Display for PasswordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLong => f.write_str("password exceeds bcrypt's 72-byte input limit"),
            Self::InvalidCost => f.write_str("bcrypt cost is outside the 4..=31 range"),
            Self::MalformedHash => {
                f.write_str("stored hash is not in bcrypt(5) modular crypt format")
            }
            Self::EntropyFailure => f.write_str("CSPRNG salt generation failed"),
        }
    }
}

impl std::error::Error for PasswordError {}

// ── CsrfToken ────────────────────────────────────────────────

/// Errors returned by [`crate::CsrfToken::parse`].
#[derive(Debug)]
#[non_exhaustive]
pub enum CsrfError {
    /// The provided string is not valid base64-url-no-pad, or
    /// decodes to a non-32-byte buffer. CSRF tokens are always
    /// exactly 32 bytes.
    MalformedEncoding,
    /// CSPRNG token generation failed (`getrandom` returned an
    /// error). Practically never happens on a healthy OS.
    EntropyFailure,
}

impl fmt::Display for CsrfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedEncoding => {
                f.write_str("CSRF token is not 32 bytes encoded as base64-url-no-pad")
            }
            Self::EntropyFailure => f.write_str("CSPRNG token generation failed"),
        }
    }
}

impl std::error::Error for CsrfError {}
