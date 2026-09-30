//! Criterion baseline for [`sentori_event_ringbuffer::Ring`].
//!
//! The ring sits on the ingest hot path — every accepted event is
//! pushed by the HTTP handler thread, and drained by a persistence
//! task. These baselines pin the working numbers for the v1 wrapper.
//!
//! ## Baseline (v1, 2026-06-20, Apple Silicon / M-series, `--quick`)
//!
//! | Bench                | Median   |
//! |----------------------|----------|
//! | `push_uncontended`   | ~6.2 ns  |
//! | `pop_uncontended`    | ~5.0 ns  |
//! | `push_under_overflow`| ~5.4 ns  |
//!
//! Single-digit-nanosecond per op — well inside any plausible ingest
//! budget. The drop-oldest policy adds no measurable cost over the
//! uncontended push thanks to `ArrayQueue`'s symmetric pop/push.

#![allow(
    missing_docs,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_panics_doc
)]

use criterion::{Criterion, criterion_group, criterion_main};
use sentori_event_ringbuffer::Ring;
use std::hint::black_box;

fn bench_push_uncontended(c: &mut Criterion) {
    let ring: Ring<u64> = Ring::with_capacity(1024).unwrap();
    c.bench_function("push_uncontended", |b| {
        // Drain when full so we measure the steady-state push path,
        // not the eviction path.
        b.iter(|| {
            if ring.is_full() {
                let _ = ring.pop();
            }
            ring.push(black_box(42));
        });
    });
}

fn bench_pop_uncontended(c: &mut Criterion) {
    let ring: Ring<u64> = Ring::with_capacity(1024).unwrap();
    c.bench_function("pop_uncontended", |b| {
        b.iter(|| {
            // Keep at least one item available each iteration.
            if ring.is_empty() {
                ring.push(7);
            }
            black_box(ring.pop());
        });
    });
}

fn bench_push_under_overflow(c: &mut Criterion) {
    // Capacity 1 forces every push past the first to evict the
    // oldest, exercising the policy hot path.
    let ring: Ring<u64> = Ring::with_capacity(1).unwrap();
    c.bench_function("push_under_overflow", |b| {
        b.iter(|| {
            black_box(ring.push(black_box(99)));
        });
    });
}

criterion_group!(
    benches,
    bench_push_uncontended,
    bench_pop_uncontended,
    bench_push_under_overflow,
);
criterion_main!(benches);
