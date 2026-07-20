//! Error types surfaced to consumers of [`crate::Issuer`] / [`crate::Verifier`].
//!
//! Designed so HTTP middleware can pattern-match on the variant and emit
//! the correct status code:
//!
//! - [`LicenseError::Expired`] → `402 Payment Required`
//! - [`LicenseError::Revoked`] → `403 Forbidden`
//! - [`LicenseError::Invalid`] / [`LicenseError::IssuerMismatch`] →
//!   `401 Unauthorized`
//! - [`LicenseError::Signing`] → server-side, `500` (never reaches client)

use thiserror::Error;

/// Convenience [`Result`] alias for license operations.
pub type LicenseResult<T> = Result<T, LicenseError>;

/// All public error states the license crate produces.
///
/// `Invalid` carries a [`String`] rather than the underlying
/// [`jsonwebtoken::errors::Error`] so the public API stays stable across
/// `jsonwebtoken` upgrades.
#[derive(Debug, Error)]
pub enum LicenseError {
    /// The token's `exp` claim is in the past (after [`Verifier`] leeway).
    ///
    /// [`Verifier`]: crate::Verifier
    #[error("license expired")]
    Expired,

    /// The token's `jti` appears in the active [`RevocationList`].
    ///
    /// [`RevocationList`]: crate::RevocationList
    #[error("license revoked")]
    Revoked,

    /// The token's `iss` claim does not match [`crate::SENTORI_ISSUER`].
    ///
    /// Surfaced as a distinct variant so middleware can log it
    /// separately from generic signature failures — a mismatched issuer
    /// often indicates a cross-deployment token replay attempt.
    #[error("license issuer mismatch")]
    IssuerMismatch,

    /// Any other validation failure (bad signature, malformed claims,
    /// disallowed algorithm, etc.). The message is for logs only;
    /// callers should not parse it.
    #[error("license invalid: {0}")]
    Invalid(String),

    /// Signing failed (issuer-side). Never produced by [`Verifier`].
    ///
    /// [`Verifier`]: crate::Verifier
    #[error("license signing failed: {0}")]
    Signing(String),
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
    fn display_messages_are_stable() {
        assert_eq!(LicenseError::Expired.to_string(), "license expired");
        assert_eq!(LicenseError::Revoked.to_string(), "license revoked");
        assert_eq!(
            LicenseError::IssuerMismatch.to_string(),
            "license issuer mismatch"
        );
        assert_eq!(
            LicenseError::Invalid("bad sig".into()).to_string(),
            "license invalid: bad sig"
        );
        assert_eq!(
            LicenseError::Signing("no key".into()).to_string(),
            "license signing failed: no key"
        );
    }

    #[test]
    fn debug_is_implemented() {
        let _ = format!("{:?}", LicenseError::Expired);
    }
}
