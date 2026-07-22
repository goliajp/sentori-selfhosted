//! Property tests for argon2-password round-trip + tamper detection.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use proptest::prelude::*;
use sentori_argon2_password::{Params, PasswordHash};

proptest! {
    #[test]
    fn hash_then_verify_round_trip(password in "\\PC{0,256}") {
        let hash = PasswordHash::hash_with_params(&password, Params::TEST_FAST)
            .expect("hash");
        prop_assert!(PasswordHash::verify(&password, &hash).expect("verify"));
    }

    #[test]
    fn distinct_passwords_do_not_collide(
        a in "\\PC{1,128}",
        b in "\\PC{1,128}"
    ) {
        prop_assume!(a != b);
        let hash = PasswordHash::hash_with_params(&a, Params::TEST_FAST).expect("hash");
        prop_assert!(!PasswordHash::verify(&b, &hash).expect("verify"));
    }

    #[test]
    fn same_password_different_hashes(password in "\\PC{0,128}") {
        let a = PasswordHash::hash_with_params(&password, Params::TEST_FAST).expect("a");
        let b = PasswordHash::hash_with_params(&password, Params::TEST_FAST).expect("b");
        prop_assert_ne!(a, b);
    }

    #[test]
    fn tampered_hash_fails_to_verify(password in "\\PC{1,128}") {
        let mut hash = PasswordHash::hash_with_params(&password, Params::TEST_FAST)
            .expect("hash");
        // Tamper with the last char (the hash payload, not the
        // metadata) — should either fail to parse or fail to verify.
        let last_idx = hash.len() - 1;
        let last_char = hash.as_bytes()[last_idx];
        let new_char = if last_char == b'A' { 'B' } else { 'A' };
        hash.replace_range(last_idx..=last_idx, &new_char.to_string());
        let outcome = PasswordHash::verify(&password, &hash);
        prop_assert!(matches!(outcome, Ok(false) | Err(_)));
    }
}
