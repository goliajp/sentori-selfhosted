//! Typed 32-byte master key + a small wrapper for the key
//! identifier that appears in every sealed envelope.

use subtle::ConstantTimeEq;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::error::{KeyIdError, KeyIdResult};

/// Fixed master-key length: 32 bytes. Both HKDF-SHA256 (used for
/// subkey derivation) and AES-256-GCM (used to wrap DEKs) work
/// natively at 32-byte input.
pub const MASTER_KEY_LEN: usize = 32;

/// A 32-byte symmetric master key. Constant-time `PartialEq`,
/// zero-on-drop via [`zeroize`].
///
/// Construction is deliberately explicit — [`MasterKey::from_bytes`]
/// takes a fixed-length array (not a slice) so the size is proven
/// at compile time. The HKDF derivation paths
/// ([`MasterKey::derive_from`] / [`MasterKey::derive_subkey`])
/// accept arbitrary input material.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct MasterKey([u8; MASTER_KEY_LEN]);

impl MasterKey {
    /// Build from a 32-byte array.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; MASTER_KEY_LEN]) -> Self {
        Self(bytes)
    }

    /// Generate a fresh random master key via the OS CSPRNG.
    ///
    /// # Errors
    ///
    /// Returns [`getrandom::Error`] if the OS RNG is unavailable
    /// — practically never on a healthy system.
    pub fn generate() -> Result<Self, getrandom::Error> {
        let mut buf = [0u8; MASTER_KEY_LEN];
        getrandom::getrandom(&mut buf)?;
        Ok(Self(buf))
    }

    /// Derive a master key from arbitrary input material via
    /// HKDF-SHA256.
    ///
    /// `salt` may be empty (HKDF treats `None` salt the same as
    /// a zero-filled salt of one hash-block length). `info` is
    /// the domain-separation tag — pick a stable, version-tagged
    /// string per crate / per purpose (e.g.
    /// `"sentori-secrets-vault-master-v1"`).
    ///
    /// # Errors
    ///
    /// None — HKDF-SHA256 expansion at 32 bytes is always valid.
    /// The signature is `Self` rather than `Result<Self, ...>`
    /// because the only failure mode (output too long) can't
    /// trigger at our fixed 32-byte output length.
    #[must_use]
    pub fn derive_from(ikm: &[u8], salt: Option<&[u8]>, info: &[u8]) -> Self {
        let hk = hkdf::Hkdf::<sha2::Sha256>::new(salt, ikm);
        let mut okm = [0u8; MASTER_KEY_LEN];
        // 32-byte output is always inside HKDF's expansion limit
        // (255 * 32 = 8160 bytes), so the Result is always Ok.
        let _ = hk.expand(info, &mut okm);
        Self(okm)
    }

    /// Derive a subkey from this master via HKDF-SHA256, using
    /// the master as IKM. Useful for per-tenant / per-purpose
    /// key isolation — compromise of one subkey doesn't leak the
    /// master or any sibling subkey.
    #[must_use]
    pub fn derive_subkey(&self, info: &[u8]) -> Self {
        Self::derive_from(&self.0, None, info)
    }

    /// Borrowed view of the raw bytes — for feeding into a
    /// cipher constructor. Do not log or serialise.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; MASTER_KEY_LEN] {
        &self.0
    }
}

impl PartialEq for MasterKey {
    fn eq(&self, other: &Self) -> bool {
        self.0.ct_eq(&other.0).into()
    }
}

impl Eq for MasterKey {}

impl core::fmt::Debug for MasterKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Never print the bytes — a forgotten log statement with
        // a MasterKey in scope shouldn't leak the key into the
        // journal.
        f.debug_struct("MasterKey")
            .field("len", &MASTER_KEY_LEN)
            .finish_non_exhaustive()
    }
}

// ── KeyId ─────────────────────────────────────────────────────

/// Maximum permitted key id length, in bytes. The envelope stores
/// the length as a single `u8` so 255 is the on-the-wire ceiling.
pub const KEY_ID_MAX_LEN: usize = 255;

/// A printable-ASCII identifier for a [`MasterKey`]. Appears in
/// the sealed envelope so callers can drive rotation by trying
/// the current key on `peek_key_id` mismatch, then falling back
/// to a legacy key.
///
/// Validated at construction:
///
/// - 1-255 bytes (the on-the-wire `u8` length cap).
/// - Printable ASCII only (`0x20`–`0x7E`). Restricts the alphabet
///   so the id is safe to drop into error messages and log lines
///   without escaping or tearing.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyId(String);

impl KeyId {
    /// Build a key id.
    ///
    /// # Errors
    ///
    /// - [`KeyIdError::Empty`] — input was empty.
    /// - [`KeyIdError::TooLong`] — input exceeded 255 bytes.
    /// - [`KeyIdError::NonPrintableAscii`] — input contained a
    ///   byte outside `0x20`–`0x7E`.
    pub fn new(s: impl Into<String>) -> KeyIdResult<Self> {
        let s: String = s.into();
        if s.is_empty() {
            return Err(KeyIdError::Empty);
        }
        if s.len() > KEY_ID_MAX_LEN {
            return Err(KeyIdError::TooLong { actual: s.len() });
        }
        if !s.bytes().all(|b| (0x20..=0x7E).contains(&b)) {
            return Err(KeyIdError::NonPrintableAscii);
        }
        Ok(Self(s))
    }

    /// Borrow the id as a `&str`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Borrow the id as bytes — for the envelope serializer.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl core::fmt::Display for KeyId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
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

    #[test]
    fn master_key_from_bytes_round_trips() {
        let bytes = [7u8; MASTER_KEY_LEN];
        let k = MasterKey::from_bytes(bytes);
        assert_eq!(k.as_bytes(), &bytes);
    }

    #[test]
    fn master_key_generate_is_random() {
        let a = MasterKey::generate().expect("ok");
        let b = MasterKey::generate().expect("ok");
        assert_ne!(a, b);
    }

    #[test]
    fn master_key_derive_from_is_deterministic() {
        let a = MasterKey::derive_from(b"hunter2", Some(b"salt"), b"info-v1");
        let b = MasterKey::derive_from(b"hunter2", Some(b"salt"), b"info-v1");
        assert_eq!(a, b);
    }

    #[test]
    fn master_key_derive_from_diverges_on_different_info() {
        let a = MasterKey::derive_from(b"hunter2", Some(b"salt"), b"info-v1");
        let b = MasterKey::derive_from(b"hunter2", Some(b"salt"), b"info-v2");
        assert_ne!(a, b);
    }

    #[test]
    fn master_key_derive_subkey_diverges_from_self() {
        let k = MasterKey::from_bytes([1u8; MASTER_KEY_LEN]);
        let sub = k.derive_subkey(b"tenant-foo");
        assert_ne!(k, sub);
    }

    #[test]
    fn master_key_derive_subkey_is_deterministic() {
        let k = MasterKey::from_bytes([1u8; MASTER_KEY_LEN]);
        let a = k.derive_subkey(b"tenant-foo");
        let b = k.derive_subkey(b"tenant-foo");
        assert_eq!(a, b);
    }

    #[test]
    fn master_key_debug_does_not_print_bytes() {
        let k = MasterKey::from_bytes([0xAB; MASTER_KEY_LEN]);
        let s = format!("{k:?}");
        assert!(s.contains("MasterKey"));
        assert!(!s.contains("0xab"));
        assert!(!s.contains("AB"));
    }

    #[test]
    fn keyid_accepts_printable() {
        let id = KeyId::new("master-v1").expect("ok");
        assert_eq!(id.as_str(), "master-v1");
    }

    #[test]
    fn keyid_rejects_empty() {
        assert!(matches!(KeyId::new(""), Err(KeyIdError::Empty)));
    }

    #[test]
    fn keyid_rejects_too_long() {
        let too_long = "x".repeat(256);
        let err = KeyId::new(too_long).expect_err("too long");
        assert!(matches!(err, KeyIdError::TooLong { actual: 256 }));
    }

    #[test]
    fn keyid_accepts_at_byte_ceiling() {
        let max = "x".repeat(255);
        assert!(KeyId::new(max).is_ok());
    }

    #[test]
    fn keyid_rejects_non_printable() {
        let err = KeyId::new("hello\x01world").expect_err("nonprintable");
        assert!(matches!(err, KeyIdError::NonPrintableAscii));
    }

    #[test]
    fn keyid_rejects_non_ascii() {
        let err = KeyId::new("hello-世界").expect_err("non ascii");
        assert!(matches!(err, KeyIdError::NonPrintableAscii));
    }
}
