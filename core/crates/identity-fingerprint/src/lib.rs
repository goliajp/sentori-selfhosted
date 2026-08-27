//! # `sentori-identity-fingerprint` — cross-project user lookup without the user
//!
//! An operator types a real email address and finds every project that
//! person has crashed in. Sentori never receives that address.
//!
//! The SDK hashes it on the device and sends the digest as
//! `user.linkHashes.email`. The server salts that digest again with a
//! secret it holds per identity scope, and stores only the result. Two
//! events belong to the same person when their stored fingerprints
//! match; nothing in the table can be turned back into an address, and
//! a stolen copy of it cannot be matched against a rainbow table
//! without the scope's salt.
//!
//! ## Algorithm (locked — matches the v1 stack byte for byte)
//!
//! ```text
//! stored = SHA256(salt || key_type || ":" || client_hash)
//! ```
//!
//! Where `client_hash` is the 64-char lowercase hex SHA-256 the SDK
//! computed, and `key_type` is `"email"`, `"phone"`, `"sub"`, …
//!
//! **This formula cannot change.** Production carries fingerprints
//! written by the v1 server; a different digest means the same person
//! stops matching their own history, silently. The tests below pin it
//! against a vector taken from that data rather than from this code.
//!
//! ## Why hash a hash
//!
//! The device-side digest alone would be a stable identifier for an
//! address across every Sentori deployment on earth — one leaked table
//! would be matchable against another. Salting per scope makes a
//! fingerprint meaningful only inside the workspace that produced it.
//!
//! ## Stone tier
//!
//! No business types, no database, no async. Two functions and a
//! validator, so the algorithm can be tested exhaustively and reused
//! anywhere the same lookup has to work.

#![forbid(unsafe_code)]

use sha2::{Digest, Sha256};

/// Length of the hex digest an SDK is expected to send.
const CLIENT_HASH_LEN: usize = 64;

/// Does this look like something the SDK hashed, rather than a raw
/// address that slipped through?
///
/// The check exists because the failure it catches is the one that
/// matters: an SDK bug (or a forged payload) putting a plaintext email
/// in `linkHashes` would have the server persist PII it promised never
/// to hold. Callers reject the whole event on `false` rather than
/// storing it anyway.
///
/// Lowercase is required, not merely accepted: `SHA256("A…")` and
/// `SHA256("a…")` differ, so tolerating both cases would file one
/// person under two fingerprints.
#[must_use]
pub fn is_valid_client_hash(s: &str) -> bool {
    s.len() == CLIENT_HASH_LEN
        && s.bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

/// The 32 bytes to store for `(scope, key_type, client_hash)`.
///
/// Callers persist the result as `BYTEA(32)`. Lookup recomputes it from
/// the operator's query and joins on equality — which is why the
/// formula is fixed rather than merely documented.
#[must_use]
pub fn compute(salt: &[u8], key_type: &str, client_hash: &str) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(salt);
    h.update(key_type.as_bytes());
    h.update(b":");
    h.update(client_hash.as_bytes());
    h.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `SHA256("alice@example.com")` — a digest the SDK would actually
    /// send, not a hand-written string. The first version of this
    /// constant was 65 characters and the validator below caught it,
    /// which is the whole reason the validator exists.
    const CLIENT: &str = "ff8d9819fc0e12bf0d24892e45987e249a28dce836a85cad60e28eaaa8c6d976";

    /// Pins the wire format. Production holds fingerprints written by
    /// the v1 server; if this vector ever needs updating, every one of
    /// them has stopped matching and cross-project lookup has silently
    /// lost its history.
    #[test]
    fn digest_is_stable() {
        let out = compute(b"\x01\x02\x03\x04", "email", CLIENT);
        assert_eq!(
            hex::encode(out),
            "0ecc90f86df2ab26eff67d27cf4c132ddaebc8d36e550eeb8a68b25033ef7c46",
        );
    }

    /// The salt is what makes a fingerprint local to one scope. Without
    /// this property a leaked table from one deployment could be
    /// matched against another.
    #[test]
    fn a_different_salt_gives_a_different_fingerprint() {
        assert_ne!(
            compute(b"salt-one", "email", CLIENT),
            compute(b"salt-two", "email", CLIENT),
        );
    }

    /// The same address reached via email and via an OAuth subject is
    /// two facts, not one; the key type has to be inside the digest or
    /// they would collide.
    #[test]
    fn key_type_participates() {
        assert_ne!(
            compute(b"salt", "email", CLIENT),
            compute(b"salt", "phone", CLIENT),
        );
    }

    /// The separator stops `("emai", "l:…")` from colliding with
    /// `("email", "…")`.
    #[test]
    fn key_type_and_hash_cannot_run_together() {
        let a = compute(b"", "ab", "c".repeat(64).as_str());
        let b = compute(b"", "a", &format!("b:{}", "c".repeat(62)));
        assert_ne!(a, b);
    }

    #[test]
    fn rejects_anything_that_is_not_a_lowercase_hex_digest() {
        assert!(is_valid_client_hash(CLIENT));
        // A raw address — the case this check exists for.
        assert!(!is_valid_client_hash("alice@example.com"));
        assert!(!is_valid_client_hash(&CLIENT.to_uppercase()));
        assert!(!is_valid_client_hash(&CLIENT[..63]));
        assert!(!is_valid_client_hash(&format!("{CLIENT}0")));
        assert!(!is_valid_client_hash(""));
        assert!(!is_valid_client_hash(&"g".repeat(64)));
    }
}
