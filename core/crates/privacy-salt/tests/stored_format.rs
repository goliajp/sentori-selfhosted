//! A fingerprint is stored. It has to be the same forever.
//!
//! `Hasher` output lands in `device_tokens.user_fingerprint_hex` and
//! is how one person's devices are recognised as one person's. If the
//! digest ever changes, every stored fingerprint stops matching the
//! ones being computed, and a send aimed at a user reaches nobody —
//! silently, because a fingerprint that matches nothing is
//! indistinguishable from a user with no devices.
//!
//! Before this file the crate's tests checked that the output was
//! sixty-four characters long. That is true of every wrong answer.

#![allow(clippy::unwrap_used, missing_docs)]

use sentori_privacy_salt::Hasher;

/// Computed by hmac 0.12 / sha2 0.10 / hkdf 0.12 on 2026-08-15.
const HASH_BY_0_12: &str = "1a5a6a2f9a8649c94f6907aeaf40d1fd02d7794a3fd237e0919cc81dfca4a520";

#[test]
fn the_digest_is_the_one_already_in_the_database() {
    let hasher = Hasher::new(&[0x11u8; 32]).unwrap();
    let tenant = uuid::Uuid::parse_str("00000000-0000-4000-8000-000000000001").unwrap();
    assert_eq!(
        hasher
            .hash_str(tenant, "user", "alice@example.com")
            .as_hex(),
        HASH_BY_0_12,
        "the fingerprint changed — every row in user_fingerprint_hex \
         now belongs to nobody"
    );
}
