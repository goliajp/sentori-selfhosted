//! Criterion benchmarks for [`sentori_license_jwt`].
//!
//! Establishes a baseline for sign / verify / revocation-check per-op
//! cost. Future regressions land in `bench-regress.yml` CI.

#![allow(
    missing_docs,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_panics_doc
)]

use criterion::{Criterion, criterion_group, criterion_main};
use ed25519_dalek::SigningKey;
use sentori_license_jwt::{Issuer, LicenseClaims, RevocationList, Tier, Verifier};
use std::hint::black_box;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

fn fixed_signing_key() -> SigningKey {
    // Deterministic so bench numbers are reproducible across runs.
    SigningKey::from_bytes(&[42u8; 32])
}

fn bench_sign(c: &mut Criterion) {
    let issuer = Issuer::new(fixed_signing_key()).expect("issuer build");
    let claims = LicenseClaims::saas_tenant(
        Uuid::nil(),
        Tier::Pro,
        "sub_bench".into(),
        OffsetDateTime::from_unix_timestamp(4_000_000_000).expect("valid timestamp"),
        Duration::days(7),
    );
    c.bench_function("sign_saas_pro", |b| {
        b.iter(|| {
            let token = issuer.sign(black_box(&claims)).expect("sign ok");
            black_box(token);
        });
    });
}

fn bench_verify(c: &mut Criterion) {
    let sk = fixed_signing_key();
    let vk = sk.verifying_key();
    let issuer = Issuer::new(sk).expect("issuer build");
    let verifier = Verifier::new(vk);
    let claims = LicenseClaims::saas_tenant(
        Uuid::nil(),
        Tier::Pro,
        "sub_bench".into(),
        OffsetDateTime::now_utc() + Duration::days(365),
        Duration::days(7),
    );
    let token = issuer.sign(&claims).expect("sign");
    c.bench_function("verify_saas_pro", |b| {
        b.iter(|| {
            let out = verifier.verify(black_box(&token)).expect("verify ok");
            black_box(out);
        });
    });
}

fn bench_revoke_lookup(c: &mut Criterion) {
    let revs: RevocationList = (0..10_000).map(|i| format!("jti-{i}")).collect();
    c.bench_function("revoke_lookup_10k_hit", |b| {
        b.iter(|| {
            let hit = revs.contains(black_box("jti-5000"));
            black_box(hit);
        });
    });
    c.bench_function("revoke_lookup_10k_miss", |b| {
        b.iter(|| {
            let hit = revs.contains(black_box("jti-not-present"));
            black_box(hit);
        });
    });
}

criterion_group!(benches, bench_sign, bench_verify, bench_revoke_lookup);
criterion_main!(benches);
