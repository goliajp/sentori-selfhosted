//! Criterion baseline for [`sentori_issue_fingerprint::Fingerprint`].
//!
//! The fingerprint computation lives on Sentori's ingest hot path —
//! every accepted event passes through it — so per-op cost matters in
//! absolute terms. These baselines lock the working numbers for the
//! v1 algorithm; CI's bench-regress gate (pending P0.2 wiring)
//! compares future runs against them.
//!
//! ## Baseline (v1, 2026-06-20, Apple Silicon / M-series, `--quick`)
//!
//! | Bench                       | Median |
//! |-----------------------------|--------|
//! | `message_short`             | ~300 ns |
//! | `message_with_dynamic_ids`  | ~520 ns |
//! | `exception_typical`         | ~440 ns |
//! | `exception_no_frame`        | ~330 ns |
//! | `degenerate`                | ~255 ns |
//! | `from_override`             | ~18 ns  |
//!
//! All paths stay sub-microsecond, well inside the ingest budget —
//! the on-the-wire ingest endpoint targets p99 < 50 ms cold per event
//! (per refactor-standards §2.1), of which fingerprinting is a single
//! step.

#![allow(
    missing_docs,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_panics_doc
)]

use criterion::{Criterion, criterion_group, criterion_main};
use sentori_issue_fingerprint::{Fingerprint, FrameSite, Input};
use std::hint::black_box;

fn bench_message_short(c: &mut Criterion) {
    c.bench_function("message_short", |b| {
        b.iter(|| {
            let fp = Fingerprint::compute(&Input::Message {
                release: black_box("myapp@1.2.3"),
                body: black_box("config reload requested"),
            });
            black_box(fp);
        });
    });
}

fn bench_message_with_dynamic_ids(c: &mut Criterion) {
    // Real-world shape — a manually-flagged message that has user IDs
    // / sequence numbers / a UUID embedded. Exercises the
    // normalisation cold path.
    c.bench_function("message_with_dynamic_ids", |b| {
        b.iter(|| {
            let fp = Fingerprint::compute(&Input::Message {
                release: black_box("myapp@1.2.3"),
                body: black_box(
                    "user 12345 hit unexpected state \
                     for session 7f3b1c8a-2e3d-4f5a-9b0c-1d2e3f4a5b6c \
                     at ts=1733567890123",
                ),
            });
            black_box(fp);
        });
    });
}

fn bench_exception_typical(c: &mut Criterion) {
    let frame = FrameSite {
        function: Some("renderHeader"),
        file: "app/screens/Home.tsx",
    };
    c.bench_function("exception_typical", |b| {
        b.iter(|| {
            let fp = Fingerprint::compute(&Input::Exception {
                release: black_box("myapp@1.2.3"),
                error_type: black_box("TypeError"),
                message: black_box("Cannot read property 'id' of undefined"),
                frame: black_box(Some(frame)),
            });
            black_box(fp);
        });
    });
}

fn bench_exception_no_frame(c: &mut Criterion) {
    c.bench_function("exception_no_frame", |b| {
        b.iter(|| {
            let fp = Fingerprint::compute(&Input::Exception {
                release: black_box("myapp@1.2.3"),
                error_type: black_box("TypeError"),
                message: black_box("Cannot read property 'id' of undefined"),
                frame: black_box(None),
            });
            black_box(fp);
        });
    });
}

fn bench_degenerate(c: &mut Criterion) {
    c.bench_function("degenerate", |b| {
        b.iter(|| {
            let fp = Fingerprint::compute(&Input::Degenerate {
                release: black_box("myapp@1.2.3"),
                kind_tag: black_box("anr"),
                seed: black_box(1_733_567_890_i64),
            });
            black_box(fp);
        });
    });
}

fn bench_override(c: &mut Criterion) {
    c.bench_function("from_override", |b| {
        b.iter(|| {
            let fp = Fingerprint::from_override(black_box("payment.card-decline")).unwrap();
            black_box(fp);
        });
    });
}

criterion_group!(
    benches,
    bench_message_short,
    bench_message_with_dynamic_ids,
    bench_exception_typical,
    bench_exception_no_frame,
    bench_degenerate,
    bench_override,
);
criterion_main!(benches);
