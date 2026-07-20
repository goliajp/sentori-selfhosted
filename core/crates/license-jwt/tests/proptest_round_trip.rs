//! Property-based round-trip tests.
//!
//! Verifies that for arbitrary, well-typed claims the issue → verify
//! cycle preserves every field. Catches drift between the JSON
//! serialization shape and the verifier's expected schema.

#![allow(
    missing_docs,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_panics_doc
)]

use ed25519_dalek::SigningKey;
use proptest::prelude::*;
use sentori_license_jwt::{
    Edition, Issuer, LicenseClaims, LicenseLimits, RevocationList, Tier, Verifier,
};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

fn arb_tier() -> impl Strategy<Value = Tier> {
    prop_oneof![
        Just(Tier::Free),
        Just(Tier::Pro),
        Just(Tier::EnterpriseCloud),
        Just(Tier::EnterpriseSelf),
    ]
}

fn arb_edition() -> impl Strategy<Value = Edition> {
    prop_oneof![Just(Edition::Saas), Just(Edition::Enterprise)]
}

fn arb_limits() -> impl Strategy<Value = LicenseLimits> {
    (
        proptest::option::of(any::<u64>()),
        proptest::option::of(any::<u32>()),
        proptest::option::of(any::<u32>()),
        proptest::option::of(any::<u32>()),
    )
        .prop_map(
            |(events_per_month, users, projects, retention_days)| LicenseLimits {
                events_per_month,
                users,
                projects,
                retention_days,
            },
        )
}

prop_compose! {
    fn arb_claims()(
        tier in arb_tier(),
        edition in arb_edition(),
        sub in "[a-zA-Z0-9_-]{1,32}",
        jti in "[a-zA-Z0-9_-]{1,40}",
        // exp in the next ~30 years so verify (without leeway adjustment)
        // never sees expired tokens.
        exp_offset_secs in 60_i64..(60_i64 * 60 * 24 * 365 * 30),
        feature_count in 0_usize..6,
        limits in arb_limits(),
        tenant_present in any::<bool>(),
        sub_id in proptest::option::of("sub_[a-zA-Z0-9]{1,16}"),
        cust_id in proptest::option::of("cus_[a-zA-Z0-9]{1,16}"),
    ) -> LicenseClaims {
        let now = OffsetDateTime::now_utc().unix_timestamp();
        let features = (0..feature_count)
            .map(|i| format!("feat_{i}"))
            .collect();
        let tenant_id = if tenant_present { Some(Uuid::now_v7()) } else { None };
        LicenseClaims {
            iss: sentori_license_jwt::SENTORI_ISSUER.to_owned(),
            sub,
            iat: now,
            exp: now + exp_offset_secs,
            jti,
            edition,
            tier,
            tenant_id,
            customer_id: cust_id,
            subscription_id: sub_id,
            features,
            limits,
        }
    }
}

fn build_pair(seed_bytes: &[u8; 32]) -> (Issuer, Verifier) {
    let sk = SigningKey::from_bytes(seed_bytes);
    let vk = sk.verifying_key();
    (Issuer::new(sk).unwrap(), Verifier::new(vk))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn round_trip_preserves_fields(claims in arb_claims(), seed in any::<[u8; 32]>()) {
        let (issuer, verifier) = build_pair(&seed);
        let token = issuer.sign(&claims).unwrap();
        let decoded = verifier.verify(&token).unwrap();
        prop_assert_eq!(decoded, claims);
    }

    #[test]
    fn revoke_blocks_specific_jti(
        claims in arb_claims(),
        seed in any::<[u8; 32]>(),
        other_jti in "[a-zA-Z0-9_-]{1,40}",
    ) {
        prop_assume!(other_jti != claims.jti);

        let sk = SigningKey::from_bytes(&seed);
        let vk = sk.verifying_key();
        let issuer = Issuer::new(sk).unwrap();
        let revs = RevocationList::new();
        let verifier = Verifier::with_revocations(vk, revs.clone());

        let token = issuer.sign(&claims).unwrap();
        verifier.verify(&token).unwrap();          // accepted pre-revoke
        revs.revoke(&other_jti[..]);                // revoke a different jti
        verifier.verify(&token).unwrap();          // still accepted
        revs.revoke(claims.jti.as_str());           // now revoke ours
        prop_assert!(verifier.verify(&token).is_err());
    }
}

#[test]
fn cross_key_token_always_rejected() {
    let (issuer_a, _) = build_pair(&[1u8; 32]);
    let (_, verifier_b) = build_pair(&[2u8; 32]);

    let claims = LicenseClaims::saas_tenant(
        Uuid::now_v7(),
        Tier::Pro,
        "sub".into(),
        OffsetDateTime::now_utc() + Duration::days(30),
        Duration::days(7),
    );
    let token = issuer_a.sign(&claims).unwrap();
    assert!(verifier_b.verify(&token).is_err());
}
