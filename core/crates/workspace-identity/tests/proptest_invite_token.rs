//! Property tests for [`sentori_workspace_identity::InviteToken`].

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use base64::Engine;
use proptest::prelude::*;
use sentori_workspace_identity::InviteToken;

proptest! {
    #[test]
    fn parse_rejects_arbitrary_length(len in 0usize..200) {
        // Skip exactly-32 — that's the success case.
        prop_assume!(len != 32);
        let bytes: Vec<u8> = (0..len).map(|i| u8::try_from(i & 0xff).unwrap_or(0)).collect();
        let wire = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&bytes);
        prop_assert!(InviteToken::parse_and_hash(&wire).is_err());
    }

    #[test]
    fn round_trip_any_token(_seed in 0u64..2_000) {
        // generate() is its own seed; loop bound just runs the
        // generate path many times to catch hidden state.
        let t = InviteToken::generate().expect("rng");
        let wire = t.to_wire_string();
        let parsed = InviteToken::parse_and_hash(&wire).expect("parse own token");
        prop_assert!(t.hash().ct_eq(&parsed));
    }

    #[test]
    fn wire_string_is_43_chars(_seed in 0u64..2_000) {
        let t = InviteToken::generate().expect("rng");
        prop_assert_eq!(t.to_wire_string().len(), 43);
    }
}
