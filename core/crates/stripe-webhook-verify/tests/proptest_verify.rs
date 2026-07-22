//! Property tests for [`sentori_stripe_webhook_verify::verify`].
//!
//! Properties under test:
//!  - Round-trip: a header signed with the same secret + body + t
//!    always verifies.
//!  - Tampering: changing the body, secret, or timestamp drops
//!    verification.
//!  - Window: drifts within tolerance accept; drifts outside reject.

#![allow(
    missing_docs,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_panics_doc,
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use hmac::{Hmac, Mac};
use proptest::prelude::*;
use sentori_stripe_webhook_verify::{Tolerance, VerifyError, verify};
use sha2::Sha256;

fn sign(t: i64, secret: &[u8], body: &[u8]) -> String {
    let ts = t.to_string();
    let mut payload = Vec::new();
    payload.extend_from_slice(ts.as_bytes());
    payload.push(b'.');
    payload.extend_from_slice(body);
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(secret).unwrap();
    mac.update(&payload);
    hex::encode(mac.finalize().into_bytes())
}

fn arb_secret() -> impl Strategy<Value = Vec<u8>> {
    proptest::collection::vec(any::<u8>(), 16..=128)
}

fn arb_body() -> impl Strategy<Value = Vec<u8>> {
    proptest::collection::vec(any::<u8>(), 0..=4096)
}

// Keep `t` in a bounded sane range so `t.abs_diff(now)` arithmetic
// stays trivial and the strategy doesn't drift into i64 corners.
fn arb_timestamp() -> impl Strategy<Value = i64> {
    -1_000_000_000_i64..=4_000_000_000_i64
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(192))]

    /// Sign-then-verify always succeeds when `now == t`.
    #[test]
    fn round_trip_accepts(
        secret in arb_secret(),
        body in arb_body(),
        t in arb_timestamp(),
    ) {
        let header = format!("t={t},v1={}", sign(t, &secret, &body));
        let verified = verify(&secret, &header, &body, t, Tolerance::default()).unwrap();
        prop_assert_eq!(verified.timestamp, t);
    }

    /// Tampering with the body breaks verification.
    #[test]
    fn body_tamper_rejects(
        secret in arb_secret(),
        body in arb_body(),
        t in arb_timestamp(),
    ) {
        let header = format!("t={t},v1={}", sign(t, &secret, &body));
        // Append one byte → body now differs.
        let mut tampered = body;
        tampered.push(0xFF);
        let err = verify(&secret, &header, &tampered, t, Tolerance::default()).unwrap_err();
        prop_assert_eq!(err, VerifyError::NoSignatureMatch);
    }

    /// Verifying with the wrong secret rejects.
    #[test]
    fn wrong_secret_rejects(
        s1 in arb_secret(),
        s2 in arb_secret(),
        body in arb_body(),
        t in arb_timestamp(),
    ) {
        prop_assume!(s1 != s2);
        let header = format!("t={t},v1={}", sign(t, &s1, &body));
        let err = verify(&s2, &header, &body, t, Tolerance::default()).unwrap_err();
        prop_assert_eq!(err, VerifyError::NoSignatureMatch);
    }

    /// Drifts within tolerance accept; drifts outside reject.
    #[test]
    fn tolerance_window_enforced(
        secret in arb_secret(),
        body in arb_body(),
        t in arb_timestamp(),
        drift in 0_i64..=600,
        window in 0_u64..=300,
    ) {
        let header = format!("t={t},v1={}", sign(t, &secret, &body));
        let now = t.checked_add(drift).unwrap_or(i64::MAX);
        let tol = Tolerance::from_seconds(window);
        let observed = verify(&secret, &header, &body, now, tol);
        let drift_u = drift as u64;
        // Bind into a local so `prop_assert!` doesn't try to use the
        // macro-input text as a format string (the `{ .. }` rest
        // pattern in `matches!` would otherwise be parsed as an
        // unterminated format spec).
        let is_window_err = matches!(
            observed,
            Err(VerifyError::TimestampOutOfWindow { .. }),
        );
        if drift_u <= window {
            prop_assert!(observed.is_ok());
        } else {
            prop_assert!(is_window_err);
        }
    }

    /// Multiple v1= entries with one matching value accept; with no
    /// matching value reject (covers the rotation path).
    #[test]
    fn rotation_with_one_match_accepts(
        secret in arb_secret(),
        body in arb_body(),
        t in arb_timestamp(),
        decoy_byte in any::<u8>(),
    ) {
        let real = sign(t, &secret, &body);
        let decoy_hex = hex::encode([decoy_byte; 32]);
        prop_assume!(decoy_hex != real);
        let header = format!("t={t},v1={decoy_hex},v1={real}");
        let verified = verify(&secret, &header, &body, t, Tolerance::default()).unwrap();
        prop_assert_eq!(verified.timestamp, t);
    }

    #[test]
    fn rotation_with_no_match_rejects(
        secret in arb_secret(),
        body in arb_body(),
        t in arb_timestamp(),
        decoy_byte in any::<u8>(),
    ) {
        let real = sign(t, &secret, &body);
        let decoy_hex = hex::encode([decoy_byte; 32]);
        prop_assume!(decoy_hex != real);
        // Two decoys, no real signature.
        let other_decoy = hex::encode([decoy_byte ^ 0xAA; 32]);
        prop_assume!(other_decoy != real);
        let header = format!("t={t},v1={decoy_hex},v1={other_decoy}");
        let err = verify(&secret, &header, &body, t, Tolerance::default()).unwrap_err();
        prop_assert_eq!(err, VerifyError::NoSignatureMatch);
    }
}
