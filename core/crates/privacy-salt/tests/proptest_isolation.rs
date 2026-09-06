//! Property tests asserting cross-(tenant, purpose, value, master)
//! isolation. Any leak would let an attacker correlate hashes across
//! the privacy boundaries the algorithm is supposed to enforce.

#![allow(
    missing_docs,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_panics_doc
)]

use proptest::prelude::*;
use sentori_privacy_salt::Hasher;
use uuid::Uuid;

fn arb_uuid() -> impl Strategy<Value = Uuid> {
    any::<[u8; 16]>().prop_map(Uuid::from_bytes)
}

fn arb_master() -> impl Strategy<Value = Vec<u8>> {
    proptest::collection::vec(any::<u8>(), 32..=128)
}

fn arb_purpose() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_]{1,32}".prop_map(Into::into)
}

fn arb_value() -> impl Strategy<Value = Vec<u8>> {
    proptest::collection::vec(any::<u8>(), 0..256)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    #[test]
    fn determinism(
        master in arb_master(),
        tenant in arb_uuid(),
        purpose in arb_purpose(),
        value in arb_value(),
    ) {
        let hasher = Hasher::new(&master).unwrap();
        let a = hasher.hash(tenant, &purpose, &value);
        let b = hasher.hash(tenant, &purpose, &value);
        prop_assert_eq!(a, b);
    }

    #[test]
    fn tenant_isolation(
        master in arb_master(),
        t1 in arb_uuid(),
        t2 in arb_uuid(),
        purpose in arb_purpose(),
        value in arb_value(),
    ) {
        prop_assume!(t1 != t2);
        let hasher = Hasher::new(&master).unwrap();
        prop_assert_ne!(
            hasher.hash(t1, &purpose, &value),
            hasher.hash(t2, &purpose, &value),
        );
    }

    #[test]
    fn purpose_isolation(
        master in arb_master(),
        tenant in arb_uuid(),
        p1 in arb_purpose(),
        p2 in arb_purpose(),
        value in arb_value(),
    ) {
        prop_assume!(p1 != p2);
        let hasher = Hasher::new(&master).unwrap();
        prop_assert_ne!(
            hasher.hash(tenant, &p1, &value),
            hasher.hash(tenant, &p2, &value),
        );
    }

    #[test]
    fn value_isolation(
        master in arb_master(),
        tenant in arb_uuid(),
        purpose in arb_purpose(),
        v1 in arb_value(),
        v2 in arb_value(),
    ) {
        prop_assume!(v1 != v2);
        let hasher = Hasher::new(&master).unwrap();
        prop_assert_ne!(
            hasher.hash(tenant, &purpose, &v1),
            hasher.hash(tenant, &purpose, &v2),
        );
    }

    #[test]
    fn master_isolation(
        m1 in arb_master(),
        m2 in arb_master(),
        tenant in arb_uuid(),
        purpose in arb_purpose(),
        value in arb_value(),
    ) {
        prop_assume!(m1 != m2);
        let h1 = Hasher::new(&m1).unwrap();
        let h2 = Hasher::new(&m2).unwrap();
        prop_assert_ne!(
            h1.hash(tenant, &purpose, &value),
            h2.hash(tenant, &purpose, &value),
        );
    }

    #[test]
    fn output_is_64_lowercase_hex(
        master in arb_master(),
        tenant in arb_uuid(),
        purpose in arb_purpose(),
        value in arb_value(),
    ) {
        let hasher = Hasher::new(&master).unwrap();
        let out = hasher.hash(tenant, &purpose, &value);
        let s = out.as_hex();
        prop_assert_eq!(s.len(), 64);
        prop_assert!(s.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }
}
