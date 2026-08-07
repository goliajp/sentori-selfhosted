//! HMAC-SHA256-sealed cookie *value*.
//!
//! Wire format: `base64url_nopad(payload || tag)` where `tag =
//! HMAC-SHA256(key, payload)`. Tag is appended (not prepended) so
//! a verifier can decode and split with a single fixed-position
//! split.
//!
//! Tamper-evident, not confidential — the payload is visible to
//! the client (and any intermediary). Use [`crate::EncryptedCookie`]
//! when the payload carries server-private state.
//!
//! ## Wire-format rationale
//!
//! - **base64-url-no-pad** instead of base64-standard or hex:
//!   stays URL-safe (Set-Cookie values can contain `=`, `;`, `,`
//!   which break parsers; `+`, `/` break URLs). The "no padding"
//!   variant trims the `==` tail that doesn't add any information.
//! - **Tag suffix** instead of prefix: `decode → split_at(len-32)`
//!   is a single pass; the alternative needs two passes (decode
//!   tag, decode payload).
//! - **Single fixed-key HMAC** instead of a key-id prefix scheme:
//!   key rotation is the caller's job (try old key on
//!   `BadSignature`, swap). Putting key-id in the cookie would
//!   leak it to the client.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;

use crate::error::{SignedCookieError, SignedCookieResult};
use crate::key::SecretKey;

type HmacSha256 = Hmac<Sha256>;

/// Length (bytes) of the appended HMAC-SHA256 tag.
pub const TAG_LEN: usize = 32;

/// Stateless namespace for the signed-cookie primitive.
///
/// Construction-free: `SignedCookie::seal(...)` / `SignedCookie::open(...)`
/// look like associated functions rather than methods on a
/// `SignedCookie` instance. That mirrors `MachoSlicer` in S7 and
/// keeps the surface minimal — there is no useful per-cookie
/// state to carry between calls.
pub struct SignedCookie;

impl SignedCookie {
    /// Seal `payload` into a tamper-evident cookie value.
    ///
    /// The returned string is base64-url-no-pad, safe to drop
    /// directly into a `Set-Cookie` header value with no further
    /// escaping.
    #[must_use]
    pub fn seal(key: &SecretKey, payload: &[u8]) -> String {
        let mut mac = HmacSha256::new_from_slice(key.as_bytes())
            // HMAC-SHA256 accepts keys of any length; the
            // `new_from_slice` Result is solely for the BLAKE-family
            // wrappers in the same trait. SecretKey guarantees a
            // 32-byte key so the panic branch is unreachable.
            .unwrap_or_else(|_| unreachable!("HMAC-SHA256 accepts 32-byte keys"));
        mac.update(payload);
        let tag = mac.finalize().into_bytes();

        let mut buf = Vec::with_capacity(payload.len() + TAG_LEN);
        buf.extend_from_slice(payload);
        buf.extend_from_slice(&tag);
        URL_SAFE_NO_PAD.encode(buf)
    }

    /// Verify `encoded` and return the original payload bytes.
    ///
    /// # Errors
    ///
    /// - [`SignedCookieError::MalformedEncoding`] — input is not
    ///   valid base64-url-no-pad or is shorter than 32 bytes.
    /// - [`SignedCookieError::BadSignature`] — HMAC tag did not
    ///   verify against the supplied key.
    pub fn open(key: &SecretKey, encoded: &str) -> SignedCookieResult<Vec<u8>> {
        let raw = URL_SAFE_NO_PAD
            .decode(encoded)
            .map_err(|_| SignedCookieError::MalformedEncoding)?;
        if raw.len() < TAG_LEN {
            return Err(SignedCookieError::MalformedEncoding);
        }
        let split = raw.len() - TAG_LEN;
        let (payload, tag) = raw.split_at(split);

        let mut mac = HmacSha256::new_from_slice(key.as_bytes())
            .unwrap_or_else(|_| unreachable!("HMAC-SHA256 accepts 32-byte keys"));
        mac.update(payload);
        let expected = mac.finalize().into_bytes();

        if !bool::from(expected.ct_eq(tag)) {
            return Err(SignedCookieError::BadSignature);
        }
        Ok(payload.to_vec())
    }
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
    use crate::key::KEY_LEN;

    fn key() -> SecretKey {
        SecretKey::from_bytes([0x42; KEY_LEN])
    }

    #[test]
    fn round_trip_small_payload() {
        let k = key();
        let cookie = SignedCookie::seal(&k, b"user-42");
        let recovered = SignedCookie::open(&k, &cookie).expect("verify");
        assert_eq!(recovered, b"user-42");
    }

    #[test]
    fn round_trip_empty_payload() {
        let k = key();
        let cookie = SignedCookie::seal(&k, b"");
        let recovered = SignedCookie::open(&k, &cookie).expect("verify");
        assert!(recovered.is_empty());
    }

    #[test]
    fn round_trip_large_payload() {
        let k = key();
        let payload = vec![0xAB; 4096];
        let cookie = SignedCookie::seal(&k, &payload);
        let recovered = SignedCookie::open(&k, &cookie).expect("verify");
        assert_eq!(recovered, payload);
    }

    #[test]
    fn round_trip_payload_with_special_bytes() {
        let k = key();
        let payload = (0..=255u8).collect::<Vec<_>>();
        let cookie = SignedCookie::seal(&k, &payload);
        let recovered = SignedCookie::open(&k, &cookie).expect("verify");
        assert_eq!(recovered, payload);
    }

    #[test]
    fn rejects_tampered_payload() {
        let k = key();
        let cookie = SignedCookie::seal(&k, b"user-42");
        // Flip a bit in the encoded value (close to start, in the
        // payload region — base64 chars map to 6-bit groups so a
        // single-char flip cleanly mutates a payload byte).
        let mut chars: Vec<char> = cookie.chars().collect();
        chars[0] = if chars[0] == 'A' { 'B' } else { 'A' };
        let tampered: String = chars.iter().collect();
        let err = SignedCookie::open(&k, &tampered).expect_err("must reject");
        assert!(matches!(err, SignedCookieError::BadSignature));
    }

    #[test]
    fn rejects_tampered_tag() {
        let k = key();
        let cookie = SignedCookie::seal(&k, b"user-42");
        let mut chars: Vec<char> = cookie.chars().collect();
        let last = chars.len() - 1;
        chars[last] = if chars[last] == 'A' { 'B' } else { 'A' };
        let tampered: String = chars.iter().collect();
        let err = SignedCookie::open(&k, &tampered).expect_err("must reject");
        assert!(matches!(err, SignedCookieError::BadSignature));
    }

    #[test]
    fn rejects_wrong_key() {
        let k1 = key();
        let k2 = SecretKey::from_bytes([0x99; KEY_LEN]);
        let cookie = SignedCookie::seal(&k1, b"user-42");
        let err = SignedCookie::open(&k2, &cookie).expect_err("wrong key");
        assert!(matches!(err, SignedCookieError::BadSignature));
    }

    #[test]
    fn rejects_garbage_string() {
        let k = key();
        let err = SignedCookie::open(&k, "not base64 at all!!!").expect_err("garbage");
        assert!(matches!(err, SignedCookieError::MalformedEncoding));
    }

    #[test]
    fn rejects_too_short() {
        let k = key();
        // 16 chars of base64-url-no-pad decode to ~12 bytes,
        // shorter than the 32-byte minimum.
        let err = SignedCookie::open(&k, "AAAAAAAAAAAAAAAA").expect_err("short");
        assert!(matches!(err, SignedCookieError::MalformedEncoding));
    }

    #[test]
    fn encoded_value_is_url_safe() {
        let k = key();
        let cookie = SignedCookie::seal(&k, b"user-42");
        // Must contain only the URL-safe alphabet (A-Za-z0-9-_).
        for ch in cookie.chars() {
            assert!(
                ch.is_ascii_alphanumeric() || ch == '-' || ch == '_',
                "non-url-safe char {ch:?} in cookie",
            );
        }
        assert!(!cookie.contains('='), "no padding expected");
    }

    #[test]
    fn distinct_payloads_yield_distinct_tags() {
        let k = key();
        let a = SignedCookie::seal(&k, b"alice");
        let b = SignedCookie::seal(&k, b"bob");
        assert_ne!(a, b);
    }

    #[test]
    fn same_payload_yields_same_encoding() {
        // HMAC is deterministic — repeated seals of the same
        // payload + key must produce identical encodings.
        let k = key();
        let a = SignedCookie::seal(&k, b"hello");
        let b = SignedCookie::seal(&k, b"hello");
        assert_eq!(a, b);
    }
}
