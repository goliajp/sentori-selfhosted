//! Stripe webhook signature verifier — header parsing + HMAC.
//!
//! The public surface is the [`verify`] free function and the
//! [`Tolerance`] / [`Verified`] companion types. Internal helpers
//! (`parse_header`, the HMAC computation) are kept private so the
//! semver surface stays minimal.

use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;

use crate::error::{VerifyError, VerifyResult};

/// Stripe's recommended default tolerance window for webhook
/// timestamps, in seconds. Equal to 300 (5 minutes).
pub const DEFAULT_TOLERANCE_SECS: u64 = 300;

/// Freshness window for the `t=` timestamp in the
/// `Stripe-Signature` header.
///
/// Replays older than `tolerance_secs` (in either direction —
/// system clock skew can push `t` slightly into the future) are
/// rejected with [`VerifyError::TimestampOutOfWindow`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tolerance {
    /// Acceptable drift between `now` and `t`, in seconds, applied
    /// symmetrically.
    pub seconds: u64,
}

impl Default for Tolerance {
    fn default() -> Self {
        Self {
            seconds: DEFAULT_TOLERANCE_SECS,
        }
    }
}

impl Tolerance {
    /// Build a tolerance from a custom window. Zero is accepted but
    /// only useful for unit testing — production callers should use
    /// at least a few tens of seconds to absorb normal clock skew.
    #[must_use]
    pub const fn from_seconds(seconds: u64) -> Self {
        Self { seconds }
    }
}

/// Outcome of a successful [`verify`] call.
///
/// Carries the Stripe-stamped `t=` timestamp so the caller can use
/// it for deduplication or audit logging without re-parsing the
/// header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Verified {
    /// The `t=` value, as a signed Unix timestamp in seconds.
    pub timestamp: i64,
}

/// Verify a `Stripe-Signature` header against the raw request body.
///
/// On success, returns the parsed timestamp wrapped in [`Verified`].
/// On failure, returns the most specific [`VerifyError`] variant
/// describing why — see that type's variants for the full list.
///
/// `now_unix` is the wall-clock time the caller observed, in Unix
/// seconds (signed because system clocks are signed). The verifier
/// does not call any clock itself so callers control the time source
/// (and so tests can pin it).
///
/// # Errors
///
/// See [`VerifyError`] — every parsing, encoding, freshness, and
/// cryptographic failure has its own variant.
pub fn verify(
    secret: &[u8],
    header: &str,
    body: &[u8],
    now_unix: i64,
    tolerance: Tolerance,
) -> VerifyResult<Verified> {
    let parsed = parse_header(header)?;

    // Freshness window. Use unsigned arithmetic on the absolute
    // drift to avoid `i64::abs` overflow on `i64::MIN`.
    let drift_secs = now_unix.abs_diff(parsed.timestamp);
    if drift_secs > tolerance.seconds {
        return Err(VerifyError::TimestampOutOfWindow {
            drift_secs,
            tolerance_secs: tolerance.seconds,
        });
    }

    // signed_payload = "<t>" + "." + body
    let ts_str = parsed.timestamp.to_string();
    let mut signed_payload = Vec::with_capacity(ts_str.len() + 1 + body.len());
    signed_payload.extend_from_slice(ts_str.as_bytes());
    signed_payload.push(b'.');
    signed_payload.extend_from_slice(body);

    let expected = hmac_sha256(secret, &signed_payload);

    // Constant-time match against every supplied v1=. Iterating
    // through all candidates (rather than short-circuiting at the
    // first byte-mismatch) keeps the timing signal flat across all
    // candidate counts — important because the *number* of `v1=`
    // entries is itself leakable when secret rotation is in
    // progress.
    let mut any_match = subtle::Choice::from(0u8);
    for sig in &parsed.signatures {
        any_match |= expected.ct_eq(sig);
    }
    if bool::from(any_match) {
        Ok(Verified {
            timestamp: parsed.timestamp,
        })
    } else {
        Err(VerifyError::NoSignatureMatch)
    }
}

#[derive(Debug)]
struct ParsedHeader {
    timestamp: i64,
    signatures: Vec<[u8; 32]>,
}

fn parse_header(header: &str) -> VerifyResult<ParsedHeader> {
    let trimmed = header.trim();
    if trimmed.is_empty() {
        return Err(VerifyError::HeaderEmpty);
    }

    let mut timestamp: Option<i64> = None;
    let mut signatures: Vec<[u8; 32]> = Vec::new();
    let mut cursor: usize = 0;

    for raw_part in trimmed.split(',') {
        let part = raw_part.trim();
        // Track the byte offset of this part inside the original
        // header so error messages can pinpoint where things broke.
        let part_start = cursor;
        cursor += raw_part.len() + 1; // +1 for the comma separator

        let Some((key, value)) = part.split_once('=') else {
            // Stripe never produces unkeyed elements, but be lenient
            // about empty / malformed segments rather than treating
            // them as fatal — only missing `t=` / `v1=` are fatal.
            continue;
        };
        let key = key.trim();
        let value = value.trim();

        match key {
            "t" => {
                let parsed = value
                    .parse::<i64>()
                    .map_err(|_| VerifyError::TimestampMalformed(value.to_owned()))?;
                timestamp = Some(parsed);
            }
            "v1" => {
                let mut buf = [0u8; 32];
                hex::decode_to_slice(value, &mut buf)
                    .map_err(|_| VerifyError::SignatureMalformed { at: part_start })?;
                signatures.push(buf);
            }
            _ => {
                // Unknown scheme tag (e.g. `v0=`) — Stripe docs
                // say accept v1 only; ignore the rest.
            }
        }
    }

    let timestamp = timestamp.ok_or(VerifyError::TimestampMissing)?;
    if signatures.is_empty() {
        return Err(VerifyError::SignaturesMissing);
    }
    Ok(ParsedHeader {
        timestamp,
        signatures,
    })
}

#[allow(clippy::expect_used)]
fn hmac_sha256(secret: &[u8], payload: &[u8]) -> [u8; 32] {
    let mut mac =
        <Hmac<Sha256> as Mac>::new_from_slice(secret).expect("HMAC-SHA256 accepts any key length");
    mac.update(payload);
    let bytes = mac.finalize().into_bytes();
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    out
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::missing_panics_doc,
    // `DEFAULT_TOLERANCE_SECS` is 300 — well inside i64 — but clippy
    // doesn't constant-fold across the cast. Allowing the lint inside
    // tests rather than threading `i64::try_from(...).unwrap()`
    // everywhere keeps the assertions readable.
    clippy::cast_possible_wrap
)]
mod tests {
    use super::*;

    const SECRET: &[u8] = b"whsec_test_secret_with_enough_bytes";
    const BODY: &[u8] = br#"{"id":"evt_123","type":"checkout.session.completed"}"#;
    const T: i64 = 1_733_567_890;

    fn sign(t: i64, secret: &[u8], body: &[u8]) -> String {
        let ts = t.to_string();
        let mut payload = Vec::new();
        payload.extend_from_slice(ts.as_bytes());
        payload.push(b'.');
        payload.extend_from_slice(body);
        let mac = hmac_sha256(secret, &payload);
        hex::encode(mac)
    }

    fn good_header() -> String {
        format!("t={T},v1={}", sign(T, SECRET, BODY))
    }

    // ---------- success ----------

    #[test]
    fn happy_path() {
        let verified = verify(SECRET, &good_header(), BODY, T, Tolerance::default()).unwrap();
        assert_eq!(verified.timestamp, T);
    }

    #[test]
    fn accepts_multiple_v1_one_match() {
        let real = sign(T, SECRET, BODY);
        let header = format!("t={T},v1=00{},v1={real}", "ab".repeat(31));
        let verified = verify(SECRET, &header, BODY, T, Tolerance::default()).unwrap();
        assert_eq!(verified.timestamp, T);
    }

    #[test]
    fn ignores_unknown_scheme_tag() {
        let real = sign(T, SECRET, BODY);
        let header = format!("t={T},v0=should_be_ignored,v1={real}");
        verify(SECRET, &header, BODY, T, Tolerance::default()).unwrap();
    }

    #[test]
    fn tolerates_header_whitespace() {
        let real = sign(T, SECRET, BODY);
        let header = format!("  t={T} , v1={real}  ");
        verify(SECRET, &header, BODY, T, Tolerance::default()).unwrap();
    }

    #[test]
    fn accepts_drift_at_tolerance_boundary() {
        let now = T + DEFAULT_TOLERANCE_SECS as i64;
        verify(SECRET, &good_header(), BODY, now, Tolerance::default()).unwrap();
    }

    #[test]
    fn accepts_drift_in_negative_direction() {
        let now = T - DEFAULT_TOLERANCE_SECS as i64;
        verify(SECRET, &good_header(), BODY, now, Tolerance::default()).unwrap();
    }

    // ---------- failure ----------

    #[test]
    fn rejects_empty_header() {
        let err = verify(SECRET, "   ", BODY, T, Tolerance::default()).unwrap_err();
        assert_eq!(err, VerifyError::HeaderEmpty);
    }

    #[test]
    fn rejects_missing_timestamp() {
        let real = sign(T, SECRET, BODY);
        let err = verify(SECRET, &format!("v1={real}"), BODY, T, Tolerance::default()).unwrap_err();
        assert_eq!(err, VerifyError::TimestampMissing);
    }

    #[test]
    fn rejects_malformed_timestamp() {
        let real = sign(T, SECRET, BODY);
        let header = format!("t=notanumber,v1={real}");
        let err = verify(SECRET, &header, BODY, T, Tolerance::default()).unwrap_err();
        assert!(matches!(err, VerifyError::TimestampMalformed(ref s) if s == "notanumber"));
    }

    #[test]
    fn rejects_missing_signatures() {
        let header = format!("t={T}");
        let err = verify(SECRET, &header, BODY, T, Tolerance::default()).unwrap_err();
        assert_eq!(err, VerifyError::SignaturesMissing);
    }

    #[test]
    fn rejects_only_unknown_schemes() {
        let header = format!("t={T},v0=abc");
        let err = verify(SECRET, &header, BODY, T, Tolerance::default()).unwrap_err();
        assert_eq!(err, VerifyError::SignaturesMissing);
    }

    #[test]
    fn rejects_malformed_signature_hex() {
        // 64-char string with non-hex content.
        let bad = "z".repeat(64);
        let header = format!("t={T},v1={bad}");
        let err = verify(SECRET, &header, BODY, T, Tolerance::default()).unwrap_err();
        assert!(matches!(err, VerifyError::SignatureMalformed { .. }));
    }

    #[test]
    fn rejects_wrong_secret() {
        let err = verify(
            b"different secret entirely",
            &good_header(),
            BODY,
            T,
            Tolerance::default(),
        )
        .unwrap_err();
        assert_eq!(err, VerifyError::NoSignatureMatch);
    }

    #[test]
    fn rejects_tampered_body() {
        let err = verify(
            SECRET,
            &good_header(),
            br#"{"id":"evt_999","type":"checkout.session.completed"}"#,
            T,
            Tolerance::default(),
        )
        .unwrap_err();
        assert_eq!(err, VerifyError::NoSignatureMatch);
    }

    #[test]
    fn rejects_old_timestamp() {
        let now = T + (DEFAULT_TOLERANCE_SECS as i64) + 1;
        let err = verify(SECRET, &good_header(), BODY, now, Tolerance::default()).unwrap_err();
        let expected_drift = (DEFAULT_TOLERANCE_SECS) + 1;
        assert_eq!(
            err,
            VerifyError::TimestampOutOfWindow {
                drift_secs: expected_drift,
                tolerance_secs: DEFAULT_TOLERANCE_SECS,
            }
        );
    }

    #[test]
    fn rejects_future_timestamp() {
        let now = T - (DEFAULT_TOLERANCE_SECS as i64) - 1;
        let err = verify(SECRET, &good_header(), BODY, now, Tolerance::default()).unwrap_err();
        assert!(matches!(err, VerifyError::TimestampOutOfWindow { .. }));
    }

    // ---------- tolerance type ----------

    #[test]
    fn tolerance_default_is_300() {
        assert_eq!(Tolerance::default().seconds, 300);
    }

    #[test]
    fn tolerance_from_seconds_round_trips() {
        let t = Tolerance::from_seconds(42);
        assert_eq!(t.seconds, 42);
    }

    #[test]
    fn zero_tolerance_requires_exact_timestamp() {
        verify(SECRET, &good_header(), BODY, T, Tolerance::from_seconds(0)).unwrap();
        let err = verify(
            SECRET,
            &good_header(),
            BODY,
            T + 1,
            Tolerance::from_seconds(0),
        )
        .unwrap_err();
        assert!(matches!(err, VerifyError::TimestampOutOfWindow { .. }));
    }

    // ---------- helper coverage ----------

    #[test]
    fn parse_header_records_signature_offset_in_error() {
        // The malformed v1= appears at byte ~ 14 (after `t=NNNNNN,`).
        let header = format!("t={T},v1=zz");
        let err = parse_header(&header).unwrap_err();
        match err {
            VerifyError::SignatureMalformed { at } => assert!(at > 0),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn parse_header_skips_keyless_segments() {
        // A bare comma in the middle of the header should not
        // explode parsing — Stripe doesn't produce these, but
        // tolerating them keeps the parser robust to whitespace /
        // proxies.
        let real = sign(T, SECRET, BODY);
        let header = format!("t={T},,v1={real}");
        verify(SECRET, &header, BODY, T, Tolerance::default()).unwrap();
    }
}
