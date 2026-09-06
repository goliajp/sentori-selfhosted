//! Criterion baseline for [`sentori_privacy_salt::Hasher`].

#![allow(
    missing_docs,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_panics_doc
)]

use criterion::{Criterion, criterion_group, criterion_main};
use sentori_privacy_salt::Hasher;
use std::hint::black_box;
use uuid::Uuid;

fn fixed_hasher() -> Hasher {
    Hasher::new(&[42u8; 64]).expect("master secret accepted")
}

fn bench_hash_small(c: &mut Criterion) {
    let hasher = fixed_hasher();
    let tenant = Uuid::from_u128(0xdead_beef_0000_0000_0000_0000_0000_0001);
    c.bench_function("hash_email_30b", |b| {
        b.iter(|| {
            let out = hasher.hash(
                black_box(tenant),
                black_box("email"),
                black_box(b"alice@example.com" as &[u8]),
            );
            black_box(out);
        });
    });
}

fn bench_hash_ip(c: &mut Criterion) {
    let hasher = fixed_hasher();
    let tenant = Uuid::from_u128(0xdead_beef_0000_0000_0000_0000_0000_0002);
    c.bench_function("hash_ip_15b", |b| {
        b.iter(|| {
            let out = hasher.hash(
                black_box(tenant),
                black_box("ip"),
                black_box(b"203.0.113.7" as &[u8]),
            );
            black_box(out);
        });
    });
}

fn bench_hash_large(c: &mut Criterion) {
    let hasher = fixed_hasher();
    let tenant = Uuid::from_u128(0xdead_beef_0000_0000_0000_0000_0000_0003);
    let payload = vec![0xa5u8; 4096];
    c.bench_function("hash_blob_4kb", |b| {
        b.iter(|| {
            let out = hasher.hash(
                black_box(tenant),
                black_box("device_id"),
                black_box(&payload[..]),
            );
            black_box(out);
        });
    });
}

criterion_group!(benches, bench_hash_small, bench_hash_ip, bench_hash_large);
criterion_main!(benches);
