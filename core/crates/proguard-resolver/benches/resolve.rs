//! Criterion benches for the proguard-resolver stone.
//!
//! Three hot paths cover realistic usage:
//!
//! 1. `parse_small` / `parse_large` — cold mapping bytes →
//!    `ParsedMapping`. Establishes cache-miss cost. Real mappings
//!    range from 100 KB (small library) to 10 MB (full app); the
//!    parse cost is dominated by the upstream's class-index build.
//! 2. `resolve_class` — repeated class lookups against a parsed
//!    mapping. Establishes per-class cost.
//! 3. `resolve_method_hot` — repeated method lookups with line
//!    information. Establishes per-frame cost for a typical
//!    20-frame Android stack symbolication.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_panics_doc,
    clippy::too_many_lines,
    clippy::format_push_string,
    missing_docs
)]

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use sentori_proguard_resolver::ParsedMapping;

fn build_mapping(n_classes: u32) -> String {
    let mut out = String::new();
    for i in 0..n_classes {
        out.push_str(&format!("com.example.pkg.Class{i} -> a{i}.b{i}:\n"));
        for j in 0..10u32 {
            let synth = 1 + j * 5;
            out.push_str(&format!("    void method{j}() -> m{j}\n"));
            out.push_str(&format!(
                "    {synth}:{end}:void method{j}():{synth}:{end} -> m{j}\n",
                end = synth + 4,
            ));
        }
    }
    out
}

fn bench_parse(c: &mut Criterion) {
    let small = build_mapping(5).into_bytes();
    let large = build_mapping(500).into_bytes();
    c.bench_function("parse_mapping_5_classes", |b| {
        b.iter(|| {
            let m = ParsedMapping::parse(black_box(small.clone())).expect("parse");
            black_box(m);
        });
    });
    c.bench_function("parse_mapping_500_classes", |b| {
        b.iter(|| {
            let m = ParsedMapping::parse(black_box(large.clone())).expect("parse");
            black_box(m);
        });
    });
}

fn bench_resolve_class(c: &mut Criterion) {
    let m = ParsedMapping::parse(build_mapping(100).into_bytes()).expect("parse");
    c.bench_function("resolve_class_hot", |b| {
        let mut i: u32 = 0;
        b.iter(|| {
            let key = format!("a{}.b{}", i % 100, i % 100);
            i = i.wrapping_add(1);
            let v = m.resolve_class(black_box(&key)).expect("ok");
            black_box(v);
        });
    });
}

fn bench_resolve_method(c: &mut Criterion) {
    let m = ParsedMapping::parse(build_mapping(100).into_bytes()).expect("parse");
    c.bench_function("resolve_method_hot", |b| {
        let mut i: u32 = 0;
        b.iter(|| {
            let class = format!("a{}.b{}", i % 100, i % 100);
            let method = format!("m{}", i % 10);
            let line = 1 + (i % 10) * 5 + 2;
            i = i.wrapping_add(1);
            let v = m
                .resolve_method(black_box(&class), black_box(&method), black_box(line))
                .expect("ok");
            black_box(v);
        });
    });
}

criterion_group!(
    benches,
    bench_parse,
    bench_resolve_class,
    bench_resolve_method
);
criterion_main!(benches);
