//! Criterion baseline for [`sentori_stripe_webhook_verify::verify`].
//!
//! Stripe webhook verification runs once per inbound webhook on the
//! billing handler — not as hot as ingest, but every Stripe event
//! incurs at minimum one HMAC-SHA256 over `(timestamp + body)`. The
//! baseline pins working numbers for the typical event-body size
//! (Stripe payloads land between 1 KB and 8 KB).
//!
//! ## Baseline (v1, 2026-06-20, Apple Silicon / M-series, `--quick`)
//!
//! | Bench                       | Median   |
//! |-----------------------------|----------|
//! | `verify_1kb`                | ~2.4 µs  |
//! | `verify_8kb`                | ~15 µs   |
//! | `verify_3_candidates_2kb`   | ~4.4 µs  |
//!
//! Hash time scales with body size (HMAC dominates above ~1 KB).
//! The 3-candidate rotation case adds ~30 ns per extra `v1=` — the
//! constant-time-compare loop is bounded by the SHA-256 work, not
//! the comparison.

#![allow(
    missing_docs,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_panics_doc
)]

use criterion::{Criterion, criterion_group, criterion_main};
use hmac::{Hmac, Mac};
use sentori_stripe_webhook_verify::{Tolerance, verify};
use sha2::Sha256;
use std::hint::black_box;

const SECRET: &[u8] = b"whsec_test_secret_with_enough_bytes_for_a_real_workload";
const T: i64 = 1_733_567_890;

fn sign(secret: &[u8], body: &[u8]) -> String {
    let ts = T.to_string();
    let mut payload = Vec::new();
    payload.extend_from_slice(ts.as_bytes());
    payload.push(b'.');
    payload.extend_from_slice(body);
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(secret).unwrap();
    mac.update(&payload);
    hex::encode(mac.finalize().into_bytes())
}

fn bench_verify_1kb(c: &mut Criterion) {
    let body = vec![b'a'; 1024];
    let header = format!("t={T},v1={}", sign(SECRET, &body));
    c.bench_function("verify_1kb", |b| {
        b.iter(|| {
            let v = verify(
                black_box(SECRET),
                black_box(&header),
                black_box(&body),
                black_box(T),
                Tolerance::default(),
            );
            let _ = black_box(v.unwrap());
        });
    });
}

fn bench_verify_8kb(c: &mut Criterion) {
    let body = vec![b'b'; 8192];
    let header = format!("t={T},v1={}", sign(SECRET, &body));
    c.bench_function("verify_8kb", |b| {
        b.iter(|| {
            let v = verify(
                black_box(SECRET),
                black_box(&header),
                black_box(&body),
                black_box(T),
                Tolerance::default(),
            );
            let _ = black_box(v.unwrap());
        });
    });
}

fn bench_verify_with_three_candidates(c: &mut Criterion) {
    // Mid-rotation header carries multiple v1= entries; only one
    // matches. Exercises the per-candidate iteration loop.
    let body = vec![b'c'; 2048];
    let real = sign(SECRET, &body);
    let header = format!(
        "t={T},v1={},v1={},v1={real}",
        "0".repeat(64),
        "1".repeat(64),
    );
    c.bench_function("verify_3_candidates_2kb", |b| {
        b.iter(|| {
            let v = verify(
                black_box(SECRET),
                black_box(&header),
                black_box(&body),
                black_box(T),
                Tolerance::default(),
            );
            let _ = black_box(v.unwrap());
        });
    });
}

criterion_group!(
    benches,
    bench_verify_1kb,
    bench_verify_8kb,
    bench_verify_with_three_candidates,
);
criterion_main!(benches);
