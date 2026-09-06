//! Typed 32-byte secret key with constant-time equality and
//! zero-on-drop.
//!
//! Distinguishes a "secret key" from a generic `&[u8]` at the type
//! level — every signing / encryption surface in the crate takes
//! [`SecretKey`] (never `&[u8]`), so call-site mistakes (passing a
//! random buffer, a user-supplied string, or a logged value) are
//! caught at compile time rather than at production time.

use core::convert::TryFrom;

use subtle::ConstantTimeEq;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// The fixed key length the crate uses: 32 bytes.
///
/// 32 is the standard for HMAC-SHA256 (block-aligned) and the
/// only AES-256-GCM key size; pinning one constant across all
/// primitives lets callers manage one key per service rather
/// than juggling several.
pub const KEY_LEN: usize = 32;

/// A 32-byte symmetric secret. Constant-time equality + zeroes
/// itself on drop (via [`zeroize`]) so a forgotten key in
/// process memory stops being a footgun.
///
/// Construction is deliberately explicit — [`SecretKey::from_bytes`]
/// takes a `[u8; 32]` value (not `&[u8]`), so the caller is forced
/// to hold an array (with the size proven at compile time) rather
/// than slicing into a possibly-shorter buffer at runtime.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SecretKey([u8; KEY_LEN]);

impl SecretKey {
    /// Build a key from a 32-byte array.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; KEY_LEN]) -> Self {
        Self(bytes)
    }

    /// Generate a fresh random key via the OS CSPRNG.
    ///
    /// # Errors
    ///
    /// Returns [`getrandom::Error`] if the OS RNG is unavailable
    /// (practically never on a healthy system).
    pub fn generate() -> Result<Self, getrandom::Error> {
        let mut buf = [0u8; KEY_LEN];
        getrandom::getrandom(&mut buf)?;
        Ok(Self(buf))
    }

    /// Expose the key as a byte slice. Use this only to feed the
    /// key into a crypto primitive (HMAC, AES-GCM); never log or
    /// serialise the result.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl PartialEq for SecretKey {
    fn eq(&self, other: &Self) -> bool {
        self.0.ct_eq(&other.0).into()
    }
}

impl Eq for SecretKey {}

impl core::fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Never print the bytes. A forgotten log statement with a
        // SecretKey in scope shouldn't leak the key into journald.
        f.debug_struct("SecretKey")
            .field("len", &KEY_LEN)
            .finish_non_exhaustive()
    }
}

/// Construction from a borrowed slice — convenient when reading a
/// key out of an env var or a config file. Returns
/// `Err(WrongLength)` if the slice is not exactly 32 bytes.
impl TryFrom<&[u8]> for SecretKey {
    type Error = WrongLength;
    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        let arr: [u8; KEY_LEN] = value.try_into().map_err(|_| WrongLength {
            actual: value.len(),
        })?;
        Ok(Self(arr))
    }
}

/// Error returned when constructing a [`SecretKey`] from a slice
/// that is not exactly [`KEY_LEN`] bytes long.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WrongLength {
    /// The actual length we got — useful for the operator-facing
    /// error message ("you gave me 16 bytes, I needed 32").
    pub actual: usize,
}

impl core::fmt::Display for WrongLength {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "SecretKey requires exactly {KEY_LEN} bytes, got {}",
            self.actual
        )
    }
}

impl std::error::Error for WrongLength {}

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
    fn from_bytes_round_trips() {
        let bytes = [7u8; KEY_LEN];
        let k = SecretKey::from_bytes(bytes);
        assert_eq!(k.as_bytes(), &bytes);
    }

    #[test]
    fn generate_produces_distinct_keys() {
        let a = SecretKey::generate().expect("ok");
        let b = SecretKey::generate().expect("ok");
        // Probability of collision = 2^-256 — effectively zero.
        assert_ne!(a, b);
    }

    #[test]
    fn eq_is_value_equal() {
        let a = SecretKey::from_bytes([3u8; KEY_LEN]);
        let b = SecretKey::from_bytes([3u8; KEY_LEN]);
        assert_eq!(a, b);
    }

    #[test]
    fn eq_rejects_differing_keys() {
        let a = SecretKey::from_bytes([3u8; KEY_LEN]);
        let mut bytes = [3u8; KEY_LEN];
        bytes[0] = 4;
        let b = SecretKey::from_bytes(bytes);
        assert_ne!(a, b);
    }

    #[test]
    fn try_from_accepts_exact_length() {
        let slice: &[u8] = &[1u8; KEY_LEN];
        let k = SecretKey::try_from(slice).expect("ok");
        assert_eq!(k.as_bytes(), slice);
    }

    #[test]
    fn try_from_rejects_short_slice() {
        let err = SecretKey::try_from(&[1u8; 16][..]).expect_err("too short");
        assert_eq!(err.actual, 16);
    }

    #[test]
    fn try_from_rejects_long_slice() {
        let err = SecretKey::try_from(&[1u8; 64][..]).expect_err("too long");
        assert_eq!(err.actual, 64);
    }

    #[test]
    fn debug_does_not_print_bytes() {
        let k = SecretKey::from_bytes([0xAB; KEY_LEN]);
        let s = format!("{k:?}");
        assert!(s.contains("SecretKey"));
        assert!(!s.contains("0xab"));
        assert!(!s.contains("AB"));
    }

    #[test]
    fn key_len_is_32() {
        assert_eq!(KEY_LEN, 32);
    }
}
