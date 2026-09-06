//! Property tests for the cache surface.
//!
//! The cache's invariants are simple but easy to drift on:
//!
//! - `len ≤ capacity` is a hard ceiling (the LRU's job, but the
//!   tests pin it down so any future cache swap respects it).
//! - `insert` then `get` returns the *same* `Arc` (no clone-and-
//!   forget; the cache must hand back a clone of the stored Arc).
//! - The read-through loader runs *iff* the key was absent.

#![allow(
    missing_docs,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_panics_doc
)]

use core::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use proptest::prelude::*;
use sentori_sourcemap_resolver::{ParsedMap, ResolverCache};
use sourcemap::SourceMapBuilder;

/// Build a single trivial parsed map — content does not matter for
/// the cache-shape properties, only that we have a real `ParsedMap`
/// to fan-out via `Arc::clone`.
fn shared_map() -> Arc<ParsedMap> {
    let mut b = SourceMapBuilder::new(Some("bundle.js"));
    let src_id = b.add_source("src/p.ts");
    b.set_source_contents(src_id, Some("const x = 1;\n"));
    b.add(0, 0, 0, 0, Some("src/p.ts"), None, false);
    let mut out = Vec::new();
    b.into_sourcemap()
        .to_writer(&mut out)
        .expect("encode shared map");
    Arc::new(ParsedMap::parse(&out).expect("parse shared map"))
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        .. ProptestConfig::default()
    })]

    /// After inserting `keys.len()` entries into a cache with cap N,
    /// the cache holds at most `min(N, keys.len())` entries (and
    /// definitely at most N).
    #[test]
    fn len_never_exceeds_capacity(
        keys in prop::collection::vec(0u32..1000, 1..200),
        cap in 1usize..50,
    ) {
        let map = shared_map();
        let c: ResolverCache<u32> =
            ResolverCache::new(NonZeroUsize::new(cap).expect("non-zero"));
        for k in &keys {
            c.insert(*k, Arc::clone(&map));
        }
        prop_assert!(c.len() <= cap);
    }

    /// `get(k)` immediately after `insert(k, v)` returns a clone of
    /// the same `Arc` (pointer-equality), provided the cache has not
    /// since evicted the entry.
    #[test]
    fn insert_then_get_roundtrips(
        key in 0u32..1000,
    ) {
        let map = shared_map();
        let c: ResolverCache<u32> =
            ResolverCache::new(NonZeroUsize::new(8).expect("non-zero"));
        c.insert(key, Arc::clone(&map));
        let got = c.get(&key).expect("just inserted");
        prop_assert!(Arc::ptr_eq(&got, &map));
    }

    /// Read-through: loader runs exactly once for the first call
    /// against a key, never again as long as the entry survives.
    #[test]
    fn read_through_loader_runs_once(
        key in 0u32..1000,
        repeats in 1usize..20,
    ) {
        let map = shared_map();
        let c: ResolverCache<u32> =
            ResolverCache::new(NonZeroUsize::new(16).expect("non-zero"));
        let calls = AtomicUsize::new(0);
        for _ in 0..repeats {
            let _ = c.get_or_try_insert_with::<_, ()>(&key, || {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(Arc::clone(&map))
            }).expect("loader ok");
        }
        prop_assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    /// `remove(k)` is idempotent — once gone, twice still gone.
    #[test]
    fn remove_is_idempotent(key in 0u32..1000) {
        let map = shared_map();
        let c: ResolverCache<u32> =
            ResolverCache::new(NonZeroUsize::new(8).expect("non-zero"));
        c.insert(key, Arc::clone(&map));
        prop_assert!(c.remove(&key).is_some());
        prop_assert!(c.remove(&key).is_none());
        prop_assert!(c.get(&key).is_none());
    }

    /// LRU eviction order is preserved: with cap=2, inserting three
    /// distinct keys evicts the first.
    #[test]
    fn lru_evicts_in_insertion_order(
        a in 0u32..100,
        b in 100u32..200,
        d in 200u32..300,
    ) {
        let map = shared_map();
        let c: ResolverCache<u32> =
            ResolverCache::new(NonZeroUsize::new(2).expect("non-zero"));
        c.insert(a, Arc::clone(&map));
        c.insert(b, Arc::clone(&map));
        c.insert(d, Arc::clone(&map));
        prop_assert!(c.get(&a).is_none(), "oldest must be evicted");
        prop_assert!(c.get(&b).is_some());
        prop_assert!(c.get(&d).is_some());
    }
}
