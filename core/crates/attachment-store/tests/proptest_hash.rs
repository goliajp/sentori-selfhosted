//! Property tests for `BlobHash` round-trips.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use proptest::prelude::*;
use sentori_attachment_store::BlobHash;

proptest! {
    #[test]
    fn hash_of_same_bytes_is_stable(bytes in proptest::collection::vec(any::<u8>(), 0..1024)) {
        let a = BlobHash::of(&bytes);
        let b = BlobHash::of(&bytes);
        prop_assert_eq!(a, b);
    }

    #[test]
    fn hash_of_different_bytes_differs(
        a in proptest::collection::vec(any::<u8>(), 1..512),
        b in proptest::collection::vec(any::<u8>(), 1..512)
    ) {
        prop_assume!(a != b);
        prop_assert_ne!(BlobHash::of(&a), BlobHash::of(&b));
    }

    #[test]
    fn hex_round_trip(bytes in proptest::collection::vec(any::<u8>(), 0..256)) {
        let h = BlobHash::of(&bytes);
        let s = h.to_hex();
        prop_assert_eq!(s.len(), 64);
        let back = BlobHash::from_hex(&s).expect("parse");
        prop_assert_eq!(back, h);
    }

    #[test]
    fn serde_round_trip(bytes in proptest::collection::vec(any::<u8>(), 0..256)) {
        let h = BlobHash::of(&bytes);
        let json = serde_json::to_string(&h).expect("ser");
        let back: BlobHash = serde_json::from_str(&json).expect("de");
        prop_assert_eq!(back, h);
    }

    #[test]
    fn rejects_random_lengths(len in 0usize..200) {
        prop_assume!(len != 64);
        let s = "a".repeat(len);
        prop_assert!(BlobHash::from_hex(&s).is_err());
    }
}
