//! # `sentori-secrets-vault` — envelope-encrypted at-rest secret storage
//!
//! Stone-tier crate (per cement-stone methodology) and the
//! twelfth and final stone of Phase 1. Designed to seal
//! individual secrets (push provider credentials, Stripe keys,
//! OAuth tokens, OIDC client secrets, anything the server stores
//! in Postgres that the operator should not be able to read with
//! a `SELECT *`).
//!
//! ## Why envelope encryption?
//!
//! The textbook two-step KMS pattern — every cloud KMS (AWS KMS,
//! GCP Cloud KMS, Azure Key Vault) uses this shape:
//!
//! 1. Generate a fresh 32-byte **data-encryption key (DEK)** per
//!    seal.
//! 2. Encrypt the plaintext with the DEK (AES-256-GCM, random
//!    nonce).
//! 3. Wrap (encrypt) the DEK with the **master key** (AES-256-GCM,
//!    random nonce).
//! 4. Persist `(key_id, wrapped_dek_nonce, wrapped_dek,
//!    payload_nonce, payload_ciphertext)` as one opaque blob.
//!
//! Compared to direct AEAD with the master (what S9
//! `EncryptedCookie` does for short-lived session payloads),
//! envelope encryption buys:
//!
//! - **Per-secret keys.** Compromise of one DEK only leaks one
//!   secret, not the whole vault.
//! - **Master rotation without re-encrypting payloads.** Rewrap
//!   each row's DEK against the new master; the payload
//!   ciphertext (which is the bulk of the data) stays untouched.
//! - **Key-id versioning.** Each sealed blob carries the master
//!   key id it was sealed under (call [`peek_key_id`] before
//!   [`Vault::open`] to route to the right vault for rotation).
//!
//! ## What this crate provides
//!
//! - [`MasterKey`] — typed 32-byte symmetric key. Zero-on-drop,
//!   constant-time equality, HKDF-SHA256 subkey derivation for
//!   per-tenant / per-purpose isolation.
//! - [`KeyId`] — printable-ASCII identifier (1-255 bytes) stamped
//!   into every sealed blob.
//! - [`Vault`] — composes `MasterKey` + `KeyId` into
//!   `seal` / `open` (plus `seal_base64` / `open_base64`).
//! - [`peek_key_id`] — standalone helper to read a sealed blob's
//!   `key_id` without unwrapping. Drives rotation logic in the
//!   钢筋 layer.
//!
//! ## On-the-wire format (version `0x01`)
//!
//! ```text
//!   offset  bytes  field
//!   ──────  ─────  ─────
//!   0       1      version (0x01)
//!   1       1      key_id_len (1..=255)
//!   2       N      key_id bytes (printable ASCII)
//!   2+N     12     wrapped_dek_nonce
//!   14+N    48     wrapped_dek (32-byte DEK + 16-byte GCM tag)
//!   62+N    12     payload_nonce
//!   74+N    ..     payload_ciphertext (plaintext + 16-byte tag)
//! ```
//!
//! Total fixed overhead: `90 + key_id_len bytes`.
//!
//! ## What this crate does NOT do
//!
//! - **No key storage / loading.** Callers hand us a `MasterKey`
//!   directly. Loading from env vars / KMS / sealed-box on disk
//!   is a 钢筋-layer concern.
//! - **No rotation orchestration.** [`peek_key_id`] is the
//!   primitive; the loop ("try current; on mismatch try legacy;
//!   rewrap on next save") lives in the K-tier.
//! - **No DB / HTTP coupling.** Sealed blobs are `Vec<u8>`
//!   (plus a base64 helper for text columns); how you persist
//!   them is your business.
//!
//! ## Concurrency model
//!
//! [`Vault`] is `Send + Sync` and stateless from the caller's
//! POV — every `seal` generates a fresh DEK + nonces, so
//! concurrent callers never share mutable state. Share via
//! `Arc<Vault>` across worker threads.
//!
//! ## Quick start
//!
//! ```rust
//! use sentori_secrets_vault::{KeyId, MasterKey, Vault, peek_key_id};
//!
//! # fn demo() -> Result<(), Box<dyn std::error::Error>> {
//! // Boot-time: load (or generate) the master + bind a key id.
//! let master = MasterKey::generate().expect("OS RNG available");
//! let vault = Vault::new(master, KeyId::new("master-v1")?);
//!
//! // Seal an APNs PEM private key for storage in `push_credentials.secret_blob`.
//! let pem = b"-----BEGIN PRIVATE KEY-----\n...";
//! let sealed = vault.seal(pem)?;
//!
//! // On a later request, unwrap.
//! assert_eq!(peek_key_id(&sealed), Some("master-v1"));
//! let recovered = vault.open(&sealed)?;
//! assert_eq!(recovered, pem);
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::multiple_crate_versions)]
#![allow(clippy::redundant_pub_crate)]
// `aes-gcm 0.10`'s `GenericArray::from_slice` is deprecated in
// favor of `generic-array 1.x` (on aes-gcm's unreleased 0.11
// milestone). Same suppression S9 cookie-session uses — the
// wire bytes are identical so the migration is trivial.
#![allow(deprecated)]

mod envelope;
mod error;
mod key;
mod vault;

pub use envelope::peek_key_id;
pub use error::{KeyIdError, KeyIdResult, OpenError, OpenResult, SealError, SealResult};
pub use key::{KEY_ID_MAX_LEN, KeyId, MASTER_KEY_LEN, MasterKey};
pub use vault::Vault;
