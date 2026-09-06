//! Property tests covering the round-trip + tampering invariants
//! of all four primitives.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_panics_doc,
    clippy::too_many_lines,
    missing_docs
)]

use proptest::prelude::*;

use sentori_cookie_session::{
    CsrfToken, EncryptedCookie, EncryptedCookieError, KEY_LEN, PasswordHash, SecretKey,
    SignedCookie, SignedCookieError,
};

fn key_strategy() -> impl Strategy<Value = SecretKey> {
    prop::array::uniform32(any::<u8>()).prop_map(SecretKey::from_bytes)
}

fn payload_strategy() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(any::<u8>(), 0..512)
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 64,
        .. ProptestConfig::default()
    })]

    /// `SignedCookie::open(key, seal(key, p)) == Ok(p)`.
    #[test]
    fn signed_round_trips(
        key in key_strategy(),
        payload in payload_strategy(),
    ) {
        let sealed = SignedCookie::seal(&key, &payload);
        let opened = SignedCookie::open(&key, &sealed).expect("open");
        prop_assert_eq!(opened, payload);
    }

    /// `SignedCookie::open` with a different key fails with
    /// `BadSignature`.
    #[test]
    fn signed_rejects_wrong_key(
        k1 in key_strategy(),
        k2 in key_strategy(),
        payload in payload_strategy(),
    ) {
        prop_assume!(k1 != k2);
        let sealed = SignedCookie::seal(&k1, &payload);
        let err = SignedCookie::open(&k2, &sealed).expect_err("wrong key");
        prop_assert!(matches!(err, SignedCookieError::BadSignature));
    }

    /// `EncryptedCookie::open(key, seal(key, p)) == Ok(p)`.
    #[test]
    fn encrypted_round_trips(
        key in key_strategy(),
        payload in payload_strategy(),
    ) {
        let sealed = EncryptedCookie::seal(&key, &payload).expect("seal");
        let opened = EncryptedCookie::open(&key, &sealed).expect("open");
        prop_assert_eq!(opened, payload);
    }

    /// `EncryptedCookie::open` with a different key fails with
    /// `Decrypt`.
    #[test]
    fn encrypted_rejects_wrong_key(
        k1 in key_strategy(),
        k2 in key_strategy(),
        payload in payload_strategy(),
    ) {
        prop_assume!(k1 != k2);
        let sealed = EncryptedCookie::seal(&k1, &payload).expect("seal");
        let err = EncryptedCookie::open(&k2, &sealed).expect_err("wrong key");
        prop_assert!(matches!(err, EncryptedCookieError::Decrypt));
    }

    /// Each `EncryptedCookie::seal` of the same (key, payload)
    /// produces a distinct ciphertext — the random per-call nonce
    /// guarantees this.
    #[test]
    fn encrypted_nonce_makes_each_seal_unique(
        key in key_strategy(),
        payload in payload_strategy(),
    ) {
        prop_assume!(!payload.is_empty());
        let a = EncryptedCookie::seal(&key, &payload).expect("seal");
        let b = EncryptedCookie::seal(&key, &payload).expect("seal");
        prop_assert_ne!(a, b);
    }

    /// CSRF token round-trip through encode/parse preserves
    /// ct_eq.
    #[test]
    fn csrf_round_trips(_seed in any::<u8>()) {
        let t = CsrfToken::generate().expect("ok");
        let wire = t.encode();
        let parsed = CsrfToken::parse(&wire).expect("parse");
        prop_assert!(t.ct_eq(&parsed));
    }

    /// Password hash with default cost round-trips for arbitrary
    /// short strings.
    #[test]
    fn password_round_trips(
        password in prop::string::string_regex("[a-zA-Z0-9!@#$%^&*]{0,32}").expect("regex"),
    ) {
        let cost = PasswordHash::COST_MIN;
        let hash = PasswordHash::hash_with_cost(&password, cost).expect("hash");
        prop_assert!(PasswordHash::verify(&password, &hash).expect("verify"));
    }

    /// `SecretKey::try_from(&[u8])` accepts iff the slice is
    /// exactly `KEY_LEN` bytes.
    #[test]
    fn key_try_from_length_invariant(
        bytes in prop::collection::vec(any::<u8>(), 0..128),
    ) {
        let result = SecretKey::try_from(bytes.as_slice());
        if bytes.len() == KEY_LEN {
            prop_assert!(result.is_ok());
        } else {
            prop_assert!(result.is_err());
        }
    }
}
