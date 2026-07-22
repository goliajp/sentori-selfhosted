//! [`Vault`] — composes a `MasterKey` + `KeyId` into the
//! `seal` / `open` API.

use aes_gcm::aead::{Aead, KeyInit, generic_array::GenericArray};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use zeroize::Zeroize;

use crate::envelope::{NONCE_LEN, WRAPPED_DEK_LEN, parse, serialise};
use crate::error::{OpenError, OpenResult, SealError, SealResult};
use crate::key::{KeyId, MasterKey};

/// Envelope-encryption vault wrapping a single [`MasterKey`].
///
/// Stateless from the caller's POV. `seal` generates a fresh DEK
/// and a pair of fresh nonces on every call; per-secret keys make
/// compromise of one DEK strictly local, and key rotation
/// re-wraps only the (small) DEKs, never the (large) payloads.
pub struct Vault {
    master: MasterKey,
    key_id: KeyId,
}

impl Vault {
    /// Build a vault. The key id is stamped into every sealed
    /// blob and required to match on open.
    #[must_use]
    pub const fn new(master: MasterKey, key_id: KeyId) -> Self {
        Self { master, key_id }
    }

    /// The key id this vault stamps into sealed blobs.
    #[must_use]
    pub const fn key_id(&self) -> &KeyId {
        &self.key_id
    }

    /// Seal `plaintext` into an opaque sealed-envelope blob.
    ///
    /// Generates a fresh 32-byte DEK + two 12-byte nonces per
    /// call (one for the DEK wrap, one for the payload). The
    /// returned bytes are safe to persist directly (e.g. into a
    /// `bytea` Postgres column) — there is no further encoding
    /// step.
    ///
    /// # Errors
    ///
    /// - [`SealError::EntropyFailure`] — OS CSPRNG refused the
    ///   DEK / nonce request.
    /// - [`SealError::EncryptFailed`] — `aes-gcm` returned an
    ///   error (effectively only OOM).
    pub fn seal(&self, plaintext: &[u8]) -> SealResult<Vec<u8>> {
        // 1. Generate per-blob DEK + nonces.
        let mut dek = [0u8; 32];
        getrandom::getrandom(&mut dek).map_err(|_| SealError::EntropyFailure)?;
        let mut wrapped_dek_nonce = [0u8; NONCE_LEN];
        getrandom::getrandom(&mut wrapped_dek_nonce).map_err(|_| SealError::EntropyFailure)?;
        let mut payload_nonce = [0u8; NONCE_LEN];
        getrandom::getrandom(&mut payload_nonce).map_err(|_| SealError::EntropyFailure)?;

        // 2. Wrap the DEK under the master.
        let master_cipher = Aes256Gcm::new(GenericArray::from_slice(self.master.as_bytes()));
        let wrapped_dek = master_cipher
            .encrypt(Nonce::from_slice(&wrapped_dek_nonce), dek.as_slice())
            .map_err(|_| SealError::EncryptFailed)?;
        debug_assert_eq!(wrapped_dek.len(), WRAPPED_DEK_LEN);

        // 3. Encrypt the payload under the fresh DEK.
        let payload_cipher = Aes256Gcm::new(GenericArray::from_slice(&dek));
        let payload_ciphertext = payload_cipher
            .encrypt(Nonce::from_slice(&payload_nonce), plaintext)
            .map_err(|_| SealError::EncryptFailed)?;

        // 4. Zero the DEK as soon as we no longer need it.
        dek.zeroize();

        // 5. Serialise envelope.
        let mut out = Vec::with_capacity(
            2 + self.key_id.as_bytes().len()
                + NONCE_LEN
                + WRAPPED_DEK_LEN
                + NONCE_LEN
                + payload_ciphertext.len(),
        );
        serialise(
            &mut out,
            self.key_id.as_bytes(),
            &wrapped_dek_nonce,
            &wrapped_dek,
            &payload_nonce,
            &payload_ciphertext,
        );
        Ok(out)
    }

    /// Seal `plaintext` and return the result base64-url-no-pad
    /// encoded. Convenience for callers that want to drop the
    /// blob into a text column / config file / response body
    /// without escaping. The raw form (from [`Self::seal`]) is
    /// preferred for binary storage.
    ///
    /// # Errors
    ///
    /// Same as [`Self::seal`].
    pub fn seal_base64(&self, plaintext: &[u8]) -> SealResult<String> {
        Ok(URL_SAFE_NO_PAD.encode(self.seal(plaintext)?))
    }

    /// Open a sealed blob. Verifies envelope shape + key id, then
    /// decrypts wrapped DEK + payload.
    ///
    /// # Errors
    ///
    /// - [`OpenError::Truncated`] / [`OpenError::UnsupportedVersion`]
    ///   / [`OpenError::InvalidKeyIdLength`] — envelope is
    ///   malformed.
    /// - [`OpenError::KeyIdMismatch`] — sealed blob's key id
    ///   doesn't match this vault's. Use [`crate::peek_key_id`]
    ///   to drive rotation lookups.
    /// - [`OpenError::WrappedDekDecryptFailed`] / [`OpenError::PayloadDecryptFailed`]
    ///   — AEAD tag mismatch; tampered ciphertext or wrong
    ///   master.
    pub fn open(&self, sealed: &[u8]) -> OpenResult<Vec<u8>> {
        let view = parse(sealed)?;
        if view.key_id != self.key_id.as_bytes() {
            return Err(OpenError::KeyIdMismatch {
                sealed_with: String::from_utf8_lossy(view.key_id).into_owned(),
                expected: self.key_id.as_str().to_owned(),
            });
        }

        // 1. Unwrap the DEK.
        let master_cipher = Aes256Gcm::new(GenericArray::from_slice(self.master.as_bytes()));
        let mut dek = master_cipher
            .decrypt(
                Nonce::from_slice(view.wrapped_dek_nonce),
                view.wrapped_dek.as_slice(),
            )
            .map_err(|_| OpenError::WrappedDekDecryptFailed)?;

        // 2. Decrypt the payload with the unwrapped DEK.
        let payload_cipher = Aes256Gcm::new(GenericArray::from_slice(&dek));
        let plaintext = payload_cipher
            .decrypt(
                Nonce::from_slice(view.payload_nonce),
                view.payload_ciphertext,
            )
            .map_err(|_| OpenError::PayloadDecryptFailed);

        // 3. Zero the DEK regardless of decrypt success.
        dek.zeroize();

        plaintext
    }

    /// Open a base64-url-no-pad encoded sealed blob. Convenience
    /// inverse of [`Self::seal_base64`].
    ///
    /// # Errors
    ///
    /// - [`OpenError::Truncated`] if the base64 decode fails or
    ///   the decoded bytes are too short. Other variants as
    ///   per [`Self::open`].
    pub fn open_base64(&self, sealed: &str) -> OpenResult<Vec<u8>> {
        let bytes = URL_SAFE_NO_PAD
            .decode(sealed)
            .map_err(|_| OpenError::Truncated)?;
        self.open(&bytes)
    }
}

impl core::fmt::Debug for Vault {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Vault")
            .field("key_id", &self.key_id)
            .field("master", &self.master)
            .finish()
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
    use crate::envelope::peek_key_id;
    use crate::key::MASTER_KEY_LEN;

    fn vault(id: &str) -> Vault {
        Vault::new(
            MasterKey::from_bytes([0x42; MASTER_KEY_LEN]),
            KeyId::new(id).expect("ok"),
        )
    }

    #[test]
    fn round_trip_small_payload() {
        let v = vault("master-v1");
        let sealed = v.seal(b"hunter2").expect("seal");
        let opened = v.open(&sealed).expect("open");
        assert_eq!(opened, b"hunter2");
    }

    #[test]
    fn round_trip_empty_payload() {
        let v = vault("master-v1");
        let sealed = v.seal(b"").expect("seal");
        assert!(v.open(&sealed).expect("open").is_empty());
    }

    #[test]
    fn round_trip_large_payload() {
        let v = vault("master-v1");
        let payload = vec![0xAB; 8 * 1024];
        let sealed = v.seal(&payload).expect("seal");
        assert_eq!(v.open(&sealed).expect("open"), payload);
    }

    #[test]
    fn distinct_seals_yield_distinct_ciphertexts() {
        // Per-call DEK + per-call nonces → identical input
        // sealed twice produces different blobs.
        let v = vault("master-v1");
        let a = v.seal(b"hello").expect("seal");
        let b = v.seal(b"hello").expect("seal");
        assert_ne!(a, b);
    }

    #[test]
    fn open_with_wrong_master_fails() {
        let v1 = Vault::new(
            MasterKey::from_bytes([0x42; MASTER_KEY_LEN]),
            KeyId::new("master-v1").expect("ok"),
        );
        let v2 = Vault::new(
            MasterKey::from_bytes([0x99; MASTER_KEY_LEN]),
            // Same id, different master — simulates a key-id
            // collision attack; the wrapped-DEK AEAD tag protects us.
            KeyId::new("master-v1").expect("ok"),
        );
        let sealed = v1.seal(b"secret").expect("seal");
        let err = v2.open(&sealed).expect_err("wrong master");
        assert!(matches!(err, OpenError::WrappedDekDecryptFailed));
    }

    #[test]
    fn open_with_mismatched_key_id_fails_distinctly() {
        let v1 = vault("master-v1");
        let v2 = Vault::new(
            MasterKey::from_bytes([0x42; MASTER_KEY_LEN]),
            KeyId::new("master-v2").expect("ok"),
        );
        let sealed = v1.seal(b"secret").expect("seal");
        let err = v2.open(&sealed).expect_err("kid mismatch");
        match err {
            OpenError::KeyIdMismatch {
                sealed_with,
                expected,
            } => {
                assert_eq!(sealed_with, "master-v1");
                assert_eq!(expected, "master-v2");
            }
            other => panic!("wrong error: {other:?}"),
        }
    }

    #[test]
    fn tampering_payload_byte_fails_payload_decrypt() {
        let v = vault("master-v1");
        let mut sealed = v.seal(b"secret").expect("seal");
        let last = sealed.len() - 1;
        sealed[last] ^= 0x01;
        let err = v.open(&sealed).expect_err("tampered");
        assert!(matches!(err, OpenError::PayloadDecryptFailed));
    }

    #[test]
    fn tampering_wrapped_dek_fails_wrapped_decrypt() {
        let v = vault("master-v1");
        let mut sealed = v.seal(b"secret").expect("seal");
        // Tamper inside the wrapped_dek field. Layout:
        //   1 ver + 1 kid_len + 9 kid("master-v1") + 12 wrapped_nonce
        //   + wrapped_dek (48 bytes here)
        // So byte index 23 is the first wrapped_dek byte.
        sealed[23] ^= 0x01;
        let err = v.open(&sealed).expect_err("tampered");
        assert!(matches!(err, OpenError::WrappedDekDecryptFailed));
    }

    #[test]
    fn peek_key_id_lets_caller_choose_vault_for_rotation() {
        let old = vault("master-v1");
        let sealed = old.seal(b"legacy secret").expect("seal");
        assert_eq!(peek_key_id(&sealed), Some("master-v1"));
        // Caller pattern:
        //   match peek_key_id(&sealed) {
        //     Some("master-v2") => current.open(&sealed),
        //     Some("master-v1") => legacy.open(&sealed),
        //     ...
        //   }
        assert_eq!(old.open(&sealed).expect("open"), b"legacy secret");
    }

    #[test]
    fn base64_round_trip() {
        let v = vault("master-v1");
        let sealed = v.seal_base64(b"secret").expect("seal");
        for ch in sealed.chars() {
            assert!(
                ch.is_ascii_alphanumeric() || ch == '-' || ch == '_',
                "non-url-safe char {ch:?}",
            );
        }
        assert_eq!(v.open_base64(&sealed).expect("open"), b"secret");
    }

    #[test]
    fn open_base64_rejects_garbage() {
        let v = vault("master-v1");
        let err = v.open_base64("not base64 !!!").expect_err("garbage");
        assert!(matches!(err, OpenError::Truncated));
    }

    #[test]
    fn debug_does_not_leak_master() {
        let v = vault("master-v1");
        let s = format!("{v:?}");
        assert!(s.contains("Vault"));
        assert!(s.contains("master-v1"));
        assert!(!s.contains("0x42"));
    }

    #[test]
    fn empty_sealed_buffer_fails_cleanly() {
        let v = vault("master-v1");
        let err = v.open(&[]).expect_err("empty");
        assert!(matches!(err, OpenError::Truncated));
    }

    #[test]
    fn key_id_accessor_round_trips() {
        let v = vault("master-v1");
        assert_eq!(v.key_id().as_str(), "master-v1");
    }
}
