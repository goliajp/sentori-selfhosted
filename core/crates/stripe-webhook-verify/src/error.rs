//! Error surface for [`crate::verify`].

use thiserror::Error;

/// Convenience [`Result`] alias.
pub type VerifyResult<T> = Result<T, VerifyError>;

/// All public failure modes the verifier can surface.
///
/// Distinct variants for distinct failure causes so callers can log
/// diagnostics (e.g. distinguish "header missing", "timestamp
/// expired", "bad signature") without forcing a string-match on
/// error text. Variants intentionally do **not** carry secret
/// material — they describe shape errors, not cryptographic
/// values.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum VerifyError {
    /// `Stripe-Signature` header was empty after trimming.
    #[error("Stripe-Signature header is empty")]
    HeaderEmpty,

    /// `Stripe-Signature` header lacked a `t=<unix>` element.
    #[error("Stripe-Signature header is missing a `t=` element")]
    TimestampMissing,

    /// The `t=` value was present but did not parse as a base-10
    /// signed integer (Stripe documents Unix seconds, signed only
    /// because i64 maps to the system clock cleanly).
    #[error("Stripe-Signature `t=` value is not a valid integer: {0:?}")]
    TimestampMalformed(String),

    /// `Stripe-Signature` header lacked any `v1=<hex>` element.
    ///
    /// Unknown scheme tags (e.g. `v0=`) are ignored when scanning, so
    /// this fires only when there is genuinely no `v1=` to consider.
    #[error("Stripe-Signature header is missing a `v1=` element")]
    SignaturesMissing,

    /// At least one `v1=` value was present but its hex decoding
    /// failed. The byte offset within the header is reported so
    /// callers can pinpoint the malformed segment.
    #[error("Stripe-Signature `v1=` value at byte {at} is not valid hex")]
    SignatureMalformed {
        /// Zero-based byte offset within the header where the bad
        /// `v1=` value starts.
        at: usize,
    },

    /// None of the supplied `v1=` signatures matched the HMAC the
    /// verifier computed from `(secret, t, body)`.
    #[error("no Stripe-Signature `v1=` value matches the HMAC of (t || \".\" || body)")]
    NoSignatureMatch,

    /// `|now - t|` exceeded the configured tolerance.
    #[error(
        "Stripe-Signature timestamp out of window: |now - t| = {drift_secs}s, tolerance {tolerance_secs}s"
    )]
    TimestampOutOfWindow {
        /// Absolute drift between `now` and the header timestamp.
        drift_secs: u64,
        /// The tolerance threshold the call was given.
        tolerance_secs: u64,
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
    fn empty_header_message() {
        assert!(VerifyError::HeaderEmpty.to_string().contains("empty"));
    }

    #[test]
    fn missing_timestamp_message() {
        assert!(
            VerifyError::TimestampMissing
                .to_string()
                .contains("missing a `t=`")
        );
    }

    #[test]
    fn malformed_timestamp_includes_value() {
        let s = VerifyError::TimestampMalformed("xyz".into()).to_string();
        assert!(s.contains("xyz"));
    }

    #[test]
    fn missing_signatures_message() {
        assert!(
            VerifyError::SignaturesMissing
                .to_string()
                .contains("missing a `v1=`")
        );
    }

    #[test]
    fn malformed_signature_includes_offset() {
        let s = VerifyError::SignatureMalformed { at: 9 }.to_string();
        assert!(s.contains('9'.to_string().as_str()));
    }

    #[test]
    fn no_match_message() {
        let s = VerifyError::NoSignatureMatch.to_string();
        assert!(s.contains("no Stripe-Signature"));
        assert!(s.contains("matches"));
    }

    #[test]
    fn out_of_window_includes_numbers() {
        let s = VerifyError::TimestampOutOfWindow {
            drift_secs: 600,
            tolerance_secs: 300,
        }
        .to_string();
        assert!(s.contains("600"));
        assert!(s.contains("300"));
    }

    #[test]
    fn debug_and_equality() {
        let a = VerifyError::HeaderEmpty;
        let b = VerifyError::HeaderEmpty;
        assert_eq!(a, b);
        let _ = format!("{a:?}");
    }
}
