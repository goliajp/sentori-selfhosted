//! Property tests for the single-use token primitive.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use base64::Engine;
use proptest::prelude::*;
use sentori_auth_session::{EmailVerifyToken, PasswordResetToken};

proptest! {
    #[test]
    fn email_verify_round_trip(_seed in 0u64..1000) {
        let t = EmailVerifyToken::generate().expect("gen");
        let wire = t.to_wire_string();
        let parsed = EmailVerifyToken::parse_and_hash(&wire).expect("parse");
        prop_assert!(t.hash().ct_eq(&parsed));
    }

    #[test]
    fn password_reset_round_trip(_seed in 0u64..1000) {
        let t = PasswordResetToken::generate().expect("gen");
        let wire = t.to_wire_string();
        let parsed = PasswordResetToken::parse_and_hash(&wire).expect("parse");
        prop_assert!(t.hash().ct_eq(&parsed));
    }

    #[test]
    fn wire_string_is_43_chars(_seed in 0u64..1000) {
        let t = EmailVerifyToken::generate().expect("gen");
        prop_assert_eq!(t.to_wire_string().len(), 43);
    }

    #[test]
    fn parse_rejects_wrong_length(len in 0usize..200) {
        prop_assume!(len != 32);
        let bytes: Vec<u8> = (0..len).map(|i| u8::try_from(i & 0xff).unwrap_or(0)).collect();
        let wire = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&bytes);
        prop_assert!(EmailVerifyToken::parse_and_hash(&wire).is_err());
    }

    #[test]
    fn distinct_tokens_distinct_hashes(_a in 0u64..1000, _b in 0u64..1000) {
        let a = EmailVerifyToken::generate().expect("a");
        let b = EmailVerifyToken::generate().expect("b");
        prop_assert!(!a.hash().ct_eq(&b.hash()));
    }
}
