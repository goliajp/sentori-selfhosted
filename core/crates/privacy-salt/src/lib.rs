//! # `sentori-privacy-salt` — per-tenant, per-purpose PII hasher
//!
//! Stone-tier crate (per cement-stone methodology) for hashing PII the
//! Sentori server-side stack must persist but never see in plaintext —
//! email addresses, IP addresses, device IDs, push tokens.
//!
//! ## Algorithm (locked v1)
//!
//! ```text
//! subkey   = HKDF-SHA256(
//!     ikm  = master_secret,                        // 32+ bytes from vault
//!     salt = tenant_id_uuid_bytes,                 // 16-byte UUID
//!     info = "sentori-privacy-salt:v1:" || purpose,
//!     okm_len = 32,
//! )
//! hash     = HMAC-SHA256(subkey, value_bytes)      // 32-byte tag
//! out      = hex(hash)                             // 64-char lowercase
//! ```
//!
//! The `info` string is versioned (`v1`) so a future algorithm change
//! never silently produces colliding output for the same `(tenant,
//! purpose, value)` triple — old + new hashes coexist behind the
//! version tag.
//!
//! ## Why HKDF + per-purpose subkey, not HMAC alone
//!
//! - **HKDF** is the right key-derivation primitive; using HMAC's `key`
//!   parameter as a salt is the well-known anti-pattern HKDF was
//!   designed to replace.
//! - **Per-purpose** subkey means a leaked email-hash table cannot be
//!   correlated against an IP-hash table even within the same tenant.
//! - **Per-tenant** salt means a leak in one tenant cannot be
//!   correlated against another tenant's hashes.
//!
//! ## Security stance
//!
//! - The crate is **not** a defence against an attacker with both the
//!   master secret and the hashed value — at that point hashing PII
//!   is irreversible only against brute-force, which is feasible for
//!   email / IP shapes. Privacy-salt narrows the attack surface; it
//!   does not eliminate it. Rotate master secrets and keep them in a
//!   vault.
//! - `Hasher` zeroizes its derived subkeys on drop via [`zeroize`].
//! - Equality compares with [`subtle::ConstantTimeEq`] when comparing
//!   raw subkey bytes (see [`Hasher::same_subkey_as`]).
//!
//! ## Quick start
//!
//! ```rust
//! use sentori_privacy_salt::Hasher;
//! use uuid::Uuid;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let master = b"32+ bytes from a real vault -- DO NOT hardcode in prod!";
//! let hasher = Hasher::new(master)?;
//!
//! let tenant = Uuid::now_v7();
//! let email_hash = hasher.hash(tenant, "email", b"alice@example.com");
//! let ip_hash    = hasher.hash(tenant, "ip",    b"203.0.113.7");
//!
//! // Stable per input
//! assert_eq!(email_hash, hasher.hash(tenant, "email", b"alice@example.com"));
//! // Different purpose → different hash
//! assert_ne!(email_hash, ip_hash);
//! // Different tenant → different hash
//! let other_tenant = Uuid::now_v7();
//! assert_ne!(email_hash, hasher.hash(other_tenant, "email", b"alice@example.com"));
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]
// Crate docs mix English narrative with technical pseudocode that the
// doc_markdown heuristic mis-flags ("HKDF", "HMAC", "PII", etc.).
#![allow(clippy::doc_markdown)]

mod error;
mod hasher;

pub use error::{PrivacySaltError, PrivacySaltResult};
pub use hasher::{Hash, Hasher, MIN_MASTER_SECRET_BYTES, OUTPUT_BYTES};

/// Versioned `info` prefix mixed into HKDF derivation — bump when the
/// algorithm changes so old and new outputs cannot collide.
pub const ALGO_VERSION_INFO: &str = "sentori-privacy-salt:v1:";
