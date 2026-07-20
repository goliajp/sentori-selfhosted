//! Criterion baseline for argon2-password.
//!
//! Measures hash + verify at `TEST_FAST` (sub-ms, for sanity),
//! `INTERACTIVE` (~50 ms, for production login), and
//! `OWASP_2025` (~100 ms, for default). The slow ones run 5
//! samples each to keep the bench under a minute.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_panics_doc,
    missing_docs
)]

use criterion::{Criterion, criterion_group, criterion_main};
use sentori_argon2_password::{Params, PasswordHash};

fn bench_hash(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash");
    group.sample_size(10);

    group.bench_function("test_fast", |b| {
        b.iter(|| PasswordHash::hash_with_params("hunter2", Params::TEST_FAST).unwrap());
    });

    group.bench_function("interactive_12mib", |b| {
        b.iter(|| PasswordHash::hash_with_params("hunter2", Params::INTERACTIVE).unwrap());
    });

    group.bench_function("owasp_2025_19mib", |b| {
        b.iter(|| PasswordHash::hash_with_params("hunter2", Params::OWASP_2025).unwrap());
    });

    group.finish();
}

fn bench_verify(c: &mut Criterion) {
    let mut group = c.benchmark_group("verify");
    group.sample_size(10);

    let fast_hash =
        PasswordHash::hash_with_params("hunter2", Params::TEST_FAST).expect("seed fast");
    let interactive_hash =
        PasswordHash::hash_with_params("hunter2", Params::INTERACTIVE).expect("seed interactive");
    let owasp_hash =
        PasswordHash::hash_with_params("hunter2", Params::OWASP_2025).expect("seed owasp");

    group.bench_function("test_fast", |b| {
        b.iter(|| PasswordHash::verify("hunter2", &fast_hash).unwrap());
    });

    group.bench_function("interactive_12mib", |b| {
        b.iter(|| PasswordHash::verify("hunter2", &interactive_hash).unwrap());
    });

    group.bench_function("owasp_2025_19mib", |b| {
        b.iter(|| PasswordHash::verify("hunter2", &owasp_hash).unwrap());
    });

    group.finish();
}

criterion_group!(benches, bench_hash, bench_verify);
criterion_main!(benches);
