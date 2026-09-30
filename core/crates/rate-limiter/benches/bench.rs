//! Criterion benches for the rate-limiter stone.
//!
//! Three hot paths cover realistic usage:
//!
//! 1. `check_allowed_hot` — repeated `check` against the same
//!    key under a generous limit; measures the steady-state
//!    per-request overhead.
//! 2. `check_distinct_keys` — repeated `check` against
//!    monotonically-increasing keys; measures the cost of growing
//!    the internal `HashMap`.
//! 3. `check_limited_path` — repeated `check` against a fully-
//!    saturated key; measures the `Limited` return path.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_panics_doc,
    clippy::too_many_lines,
    missing_docs
)]

use core::time::Duration;
use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use sentori_rate_limiter::{Limiter, MemoryBackend, Policy, SystemClock};

fn bench_check_allowed(c: &mut Criterion) {
    let policy = Policy::new(1_000_000, Duration::from_mins(1)).expect("policy");
    let limiter = Limiter::new(MemoryBackend::new(), SystemClock, policy);
    c.bench_function("check_allowed_hot", |b| {
        b.iter(|| {
            let v = limiter.check(black_box("user-42"));
            black_box(v);
        });
    });
}

fn bench_check_distinct_keys(c: &mut Criterion) {
    let policy = Policy::new(10, Duration::from_mins(1)).expect("policy");
    let limiter = Limiter::new(MemoryBackend::new(), SystemClock, policy);
    c.bench_function("check_distinct_keys", |b| {
        let mut i: u64 = 0;
        b.iter(|| {
            let key = format!("user-{}", i % 100_000);
            i = i.wrapping_add(1);
            let v = limiter.check(black_box(&key));
            black_box(v);
        });
    });
}

fn bench_check_limited(c: &mut Criterion) {
    let policy = Policy::per_minute(1).expect("policy");
    let limiter = Limiter::new(MemoryBackend::new(), SystemClock, policy);
    // Saturate the key first.
    limiter.check("k");
    c.bench_function("check_limited_path", |b| {
        b.iter(|| {
            let v = limiter.check(black_box("k"));
            black_box(v);
        });
    });
}

criterion_group!(
    benches,
    bench_check_allowed,
    bench_check_distinct_keys,
    bench_check_limited
);
criterion_main!(benches);
