//! Criterion benches for the secrets-vault stone.
//!
//! Four hot paths:
//!
//! 1. `seal_small_128b` / `seal_large_8k` — envelope-encrypt
//!    cost. Two AEAD encrypts (DEK wrap + payload encrypt) +
//!    serialise.
//! 2. `open_small_128b` / `open_large_8k` — envelope-decrypt
//!    cost. Two AEAD decrypts in sequence.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_panics_doc,
    missing_docs
)]

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use sentori_secrets_vault::{KeyId, MASTER_KEY_LEN, MasterKey, Vault};

fn build_vault() -> Vault {
    Vault::new(
        MasterKey::from_bytes([0x42; MASTER_KEY_LEN]),
        KeyId::new("master-v1").expect("ok"),
    )
}

fn bench_seal(c: &mut Criterion) {
    let v = build_vault();
    let small = vec![0u8; 128];
    let large = vec![0u8; 8 * 1024];

    c.bench_function("seal_small_128b", |b| {
        b.iter(|| {
            let s = v.seal(black_box(&small)).expect("seal");
            black_box(s);
        });
    });
    c.bench_function("seal_large_8k", |b| {
        b.iter(|| {
            let s = v.seal(black_box(&large)).expect("seal");
            black_box(s);
        });
    });
}

fn bench_open(c: &mut Criterion) {
    let v = build_vault();
    let small_sealed = v.seal(&[0u8; 128]).expect("seal");
    let large_sealed = v.seal(&vec![0u8; 8 * 1024]).expect("seal");

    c.bench_function("open_small_128b", |b| {
        b.iter(|| {
            let p = v.open(black_box(&small_sealed)).expect("open");
            black_box(p);
        });
    });
    c.bench_function("open_large_8k", |b| {
        b.iter(|| {
            let p = v.open(black_box(&large_sealed)).expect("open");
            black_box(p);
        });
    });
}

criterion_group!(benches, bench_seal, bench_open);
criterion_main!(benches);
