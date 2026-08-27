//! Criterion benches for the cookie-session stone.
//!
//! Five hot paths cover realistic usage:
//!
//! 1. `signed_seal` / `signed_open` — HMAC sign / verify.
//! 2. `encrypted_seal` / `encrypted_open` — AES-GCM encrypt /
//!    decrypt.
//! 3. `password_hash_min_cost` / `password_verify_min_cost` —
//!    bcrypt at the minimum cost (sub-ms; the per-cost knob
//!    scales exponentially so we don't bench higher costs).
//! 4. `csrf_generate` / `csrf_round_trip` — CSPRNG + encode +
//!    parse.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_panics_doc,
    missing_docs
)]

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use sentori_cookie_session::{
    CsrfToken, EncryptedCookie, KEY_LEN, PasswordHash, SecretKey, SignedCookie,
};

fn bench_signed(c: &mut Criterion) {
    let key = SecretKey::from_bytes([0x42; KEY_LEN]);
    let payload = vec![0u8; 128];
    let sealed = SignedCookie::seal(&key, &payload);

    c.bench_function("signed_seal_128b", |b| {
        b.iter(|| {
            let s = SignedCookie::seal(black_box(&key), black_box(&payload));
            black_box(s);
        });
    });

    c.bench_function("signed_open_128b", |b| {
        b.iter(|| {
            let p = SignedCookie::open(black_box(&key), black_box(&sealed)).expect("ok");
            black_box(p);
        });
    });
}

fn bench_encrypted(c: &mut Criterion) {
    let key = SecretKey::from_bytes([0x42; KEY_LEN]);
    let payload = vec![0u8; 128];
    let sealed = EncryptedCookie::seal(&key, &payload).expect("seal");

    c.bench_function("encrypted_seal_128b", |b| {
        b.iter(|| {
            let s = EncryptedCookie::seal(black_box(&key), black_box(&payload)).expect("ok");
            black_box(s);
        });
    });

    c.bench_function("encrypted_open_128b", |b| {
        b.iter(|| {
            let p = EncryptedCookie::open(black_box(&key), black_box(&sealed)).expect("ok");
            black_box(p);
        });
    });
}

fn bench_password(c: &mut Criterion) {
    // Cost 4 = minimum bcrypt accepts; bench-time is still ~1ms
    // here but stays interactive. Higher costs scale 2^n so we
    // don't run them in the harness.
    let cost = PasswordHash::COST_MIN;
    let stored = PasswordHash::hash_with_cost("hunter2", cost).expect("hash");

    c.bench_function("password_hash_min_cost", |b| {
        b.iter(|| {
            let h = PasswordHash::hash_with_cost(black_box("hunter2"), cost).expect("hash");
            black_box(h);
        });
    });

    c.bench_function("password_verify_min_cost", |b| {
        b.iter(|| {
            let v = PasswordHash::verify(black_box("hunter2"), black_box(&stored)).expect("verify");
            black_box(v);
        });
    });
}

fn bench_csrf(c: &mut Criterion) {
    let t = CsrfToken::generate().expect("ok");
    let wire = t.encode();

    c.bench_function("csrf_generate", |b| {
        b.iter(|| {
            let t = CsrfToken::generate().expect("ok");
            black_box(t);
        });
    });

    c.bench_function("csrf_parse", |b| {
        b.iter(|| {
            let t = CsrfToken::parse(black_box(&wire)).expect("ok");
            black_box(t);
        });
    });
}

criterion_group!(
    benches,
    bench_signed,
    bench_encrypted,
    bench_password,
    bench_csrf
);
criterion_main!(benches);
