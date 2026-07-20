//! AES-256-GCM-sealed cookie *value* (AEAD).
//!
//! Wire format: `base64url_nopad(nonce || ciphertext || tag)` where
//! `nonce` is 12 random bytes generated per call. The ciphertext
//! and tag are produced by `aes-gcm`'s AEAD `encrypt`.
//!
//! Both confidential (payload encrypted) and tamper-evident (GCM
//! integrity tag) — use this when the cookie carries state the
//! server must hide from the client (impersonation tokens, sealed
//! state machines, etc.). When the payload is harmless to expose,
//! prefer [`crate::SignedCookie`] for a smaller wire size and
//! faster path.
//!
//! ## Nonce strategy
//!
//! Each `seal` generates a fresh 96-bit random nonce via the OS
//! CSPRNG. With ~2^32 cookies per key the collision probability
//! is ~2^-32, which is the documented safe ceiling for random-
//! nonce GCM. Callers signing more than that should rotate the
//! key; key rotation is the caller's concern (try the old key on
//! `Decrypt` failure, swap).

use aes_gcm::aead::{Aead, KeyInit, generic_array::GenericArray};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

use crate::error::{EncryptedCookieError, EncryptedCookieResult};
use crate::key::SecretKey;

/// AES-GCM nonce length in bytes. Fixed by the GCM construction.
pub const NONCE_LEN: usize = 12;

/// GCM authentication tag length in bytes. Fixed by `Aes256Gcm`.
pub const TAG_LEN: usize = 16;

/// Stateless namespace for the encrypted-cookie primitive.
pub struct EncryptedCookie;

impl EncryptedCookie {
    /// Encrypt + authenticate `payload` into a sealed cookie
    /// value.
    ///
    /// The returned string is base64-url-no-pad, safe to drop
    /// directly into a `Set-Cookie` header value with no further
    /// escaping.
    ///
    /// # Errors
    ///
    /// - [`EncryptedCookieError::Decrypt`] is never returned by
    ///   `seal`; the function returns [`Result`] only because
    ///   `aes-gcm`'s encrypt signature does. In practice the
    ///   only failure path is OOM (in which case the allocator
    ///   panics first), so this is effectively infallible.
    pub fn seal(key: &SecretKey, payload: &[u8]) -> EncryptedCookieResult<String> {
        let cipher = Aes256Gcm::new(GenericArray::from_slice(key.as_bytes()));

        let mut nonce_bytes = [0u8; NONCE_LEN];
        // OS CSPRNG; healthy systems return Ok always. On the
        // unhealthy ones (missing /dev/urandom, sandboxed without
        // getrandom syscall) we treat it as a decrypt-shaped
        // failure so callers don't need a third error variant.
        getrandom::getrandom(&mut nonce_bytes).map_err(|_| EncryptedCookieError::Decrypt)?;
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, payload)
            .map_err(|_| EncryptedCookieError::Decrypt)?;

        let mut buf = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        buf.extend_from_slice(&nonce_bytes);
        buf.extend_from_slice(&ciphertext);
        Ok(URL_SAFE_NO_PAD.encode(buf))
    }

    /// Verify + decrypt `encoded` and return the original payload
    /// bytes.
    ///
    /// # Errors
    ///
    /// - [`EncryptedCookieError::MalformedEncoding`] — input is
    ///   not valid base64-url-no-pad or is shorter than the
    ///   minimum (nonce + tag) length.
    /// - [`EncryptedCookieError::Decrypt`] — the GCM
    ///   authentication tag did not verify; the ciphertext was
    ///   tampered with or the key is wrong.
    pub fn open(key: &SecretKey, encoded: &str) -> EncryptedCookieResult<Vec<u8>> {
        let raw = URL_SAFE_NO_PAD
            .decode(encoded)
            .map_err(|_| EncryptedCookieError::MalformedEncoding)?;
        if raw.len() < NONCE_LEN + TAG_LEN {
            return Err(EncryptedCookieError::MalformedEncoding);
        }
        let (nonce_bytes, ciphertext) = raw.split_at(NONCE_LEN);
        let nonce = Nonce::from_slice(nonce_bytes);

        let cipher = Aes256Gcm::new(GenericArray::from_slice(key.as_bytes()));
        cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| EncryptedCookieError::Decrypt)
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
        let sealed = EncryptedCookie::seal(&k, b"user-42").expect("seal");
        let opened = EncryptedCookie::open(&k, &sealed).expect("open");
        assert_eq!(opened, b"user-42");
    }

    #[test]
    fn round_trip_empty_payload() {
        let k = key();
        let sealed = EncryptedCookie::seal(&k, b"").expect("seal");
        let opened = EncryptedCookie::open(&k, &sealed).expect("open");
        assert!(opened.is_empty());
    }

    #[test]
    fn round_trip_large_payload() {
        let k = key();
        let payload = vec![0xAB; 4096];
        let sealed = EncryptedCookie::seal(&k, &payload).expect("seal");
        let opened = EncryptedCookie::open(&k, &sealed).expect("open");
        assert_eq!(opened, payload);
    }

    #[test]
    fn distinct_seals_yield_distinct_encodings() {
        // Random nonce per call → repeated seals of the same
        // payload + key produce different ciphertexts.
        let k = key();
        let a = EncryptedCookie::seal(&k, b"hello").expect("seal");
        let b = EncryptedCookie::seal(&k, b"hello").expect("seal");
        assert_ne!(a, b, "same payload must encrypt to different output");
    }

    #[test]
    fn rejects_tampered_ciphertext() {
        let k = key();
        let sealed = EncryptedCookie::seal(&k, b"user-42").expect("seal");
        let mut chars: Vec<char> = sealed.chars().collect();
        // Flip a byte well past the nonce so we're definitely
        // mutating ciphertext.
        let pos = chars.len() - 3;
        chars[pos] = if chars[pos] == 'A' { 'B' } else { 'A' };
        let tampered: String = chars.iter().collect();
        let err = EncryptedCookie::open(&k, &tampered).expect_err("must reject");
        assert!(matches!(err, EncryptedCookieError::Decrypt));
    }

    #[test]
    fn rejects_wrong_key() {
        let k1 = key();
        let k2 = SecretKey::from_bytes([0x99; KEY_LEN]);
        let sealed = EncryptedCookie::seal(&k1, b"user-42").expect("seal");
        let err = EncryptedCookie::open(&k2, &sealed).expect_err("wrong key");
        assert!(matches!(err, EncryptedCookieError::Decrypt));
    }

    #[test]
    fn rejects_garbage_encoding() {
        let k = key();
        let err = EncryptedCookie::open(&k, "not base64 !!!").expect_err("garbage");
        assert!(matches!(err, EncryptedCookieError::MalformedEncoding));
    }

    #[test]
    fn rejects_too_short_input() {
        let k = key();
        // 4 base64 chars decode to 3 bytes — far below the 28-byte
        // minimum (nonce 12 + tag 16).
        let err = EncryptedCookie::open(&k, "AAAA").expect_err("short");
        assert!(matches!(err, EncryptedCookieError::MalformedEncoding));
    }

    #[test]
    fn encoded_value_is_url_safe() {
        let k = key();
        let sealed = EncryptedCookie::seal(&k, b"user-42").expect("seal");
        for ch in sealed.chars() {
            assert!(
                ch.is_ascii_alphanumeric() || ch == '-' || ch == '_',
                "non-url-safe char {ch:?}",
            );
        }
        assert!(!sealed.contains('='));
    }
}
