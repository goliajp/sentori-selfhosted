//! Criterion benches for the sourcemap-resolver stone.
//!
//! Three hot paths cover the realistic shapes of usage:
//!
//! 1. `parse_small` / `parse_large` — cold `.map` bytes →
//!    [`ParsedMap`]. Establishes the cost a cache miss pays.
//! 2. `resolve_hot` — repeated `(line, column)` lookups against
//!    a pre-parsed map. Establishes the per-frame cost a typical
//!    20-frame stack symbolication pays once the map is hot.
//! 3. `cache_get_or_insert` — read-through cache, all-hit. Models
//!    the steady-state ingest path where the same release tends to
//!    own the next 1000 events.

#![allow(
    missing_docs,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_panics_doc,
    clippy::format_collect
)]

use core::num::NonZeroUsize;
use std::hint::black_box;
use std::sync::Arc;

use criterion::{Criterion, criterion_group, criterion_main};
use sentori_sourcemap_resolver::{ParsedMap, ResolverCache};
use sourcemap::SourceMapBuilder;

/// Build a synthetic map with `n` tokens spread across one bundle
/// line, mimicking a one-line minified bundle that concatenates `n`
/// source statements.
fn make_map(n: u32) -> Vec<u8> {
    let mut b = SourceMapBuilder::new(Some("bundle.js"));
    let src_id = b.add_source("src/synthetic.ts");
    let content: String = (0..n)
        .map(|i| format!("const fn{i} = () => {i};\n"))
        .collect();
    b.set_source_contents(src_id, Some(&content));
    for i in 0..n {
        // bundle line 0, columns spaced by 32; map to line i,
        // column 6 (after `const `) in the source.
        b.add(0, i * 32, i, 6, Some("src/synthetic.ts"), Some("fn"), false);
    }
    let mut out = Vec::new();
    b.into_sourcemap()
        .to_writer(&mut out)
        .expect("encode synthetic map");
    out
}

fn bench_parse(c: &mut Criterion) {
    let small = make_map(50);
    let large = make_map(5_000);
    c.bench_function("parse_small_50_tokens", |b| {
        b.iter(|| {
            let m = ParsedMap::parse(black_box(&small)).expect("parse small");
            black_box(m);
        });
    });
    c.bench_function("parse_large_5000_tokens", |b| {
        b.iter(|| {
            let m = ParsedMap::parse(black_box(&large)).expect("parse large");
            black_box(m);
        });
    });
}

fn bench_resolve(c: &mut Criterion) {
    let bytes = make_map(5_000);
    let map = ParsedMap::parse(&bytes).expect("parse for resolve");
    c.bench_function("resolve_hot_lookup", |b| {
        let mut i: u32 = 0;
        b.iter(|| {
            // bundle line 1 (1-indexed), columns 0, 32, 64, … —
            // every iteration hits a different token, defeating any
            // accidental constant-folding the optimiser might try.
            let col = (i % 5_000) * 32;
            i = i.wrapping_add(1);
            let r = map.resolve(black_box(1), black_box(col));
            black_box(r);
        });
    });
}

fn bench_cache(c: &mut Criterion) {
    let bytes = make_map(500);
    let parsed = Arc::new(ParsedMap::parse(&bytes).expect("parse for cache bench"));
    let cache: ResolverCache<u32> = ResolverCache::new(NonZeroUsize::new(64).expect("non-zero"));
    cache.insert(7, Arc::clone(&parsed));

    c.bench_function("cache_get_all_hit", |b| {
        b.iter(|| {
            let v = cache.get(black_box(&7)).expect("hit");
            black_box(v);
        });
    });
    let key: u32 = 7;
    c.bench_function("cache_read_through_all_hit", |b| {
        b.iter(|| {
            let v = cache
                .get_or_try_insert_with::<_, ()>(black_box(&key), || Ok(Arc::clone(&parsed)))
                .expect("loader ok");
            black_box(v);
        });
    });
}

criterion_group!(benches, bench_parse, bench_resolve, bench_cache);
criterion_main!(benches);
