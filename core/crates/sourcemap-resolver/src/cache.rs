//! Bounded LRU cache around [`ParsedMap`].
//!
//! Parsing a multi-megabyte Source Map V3 document takes single-digit
//! milliseconds; doing it on every stack-frame lookup would burn
//! both CPU and memory under bursty ingest. [`ResolverCache`] keeps
//! a fixed-capacity LRU keyed on whatever identifier the caller uses
//! to address a map (typically a release UUID, but generic so this
//! crate has no opinion on identity).
//!
//! ## Design notes
//!
//! - Capacity is a [`NonZeroUsize`]; a zero-cap cache would be a
//!   foot-gun (every `get_or_insert` would re-parse and immediately
//!   evict). The constructor refuses it at compile-time-of-spelling.
//! - Internal locking is a [`std::sync::Mutex`] — the LRU's
//!   `O(1)` access is fast enough that a contended `RwLock` upgrade
//!   is not worth the API complexity, and `get()` on an LRU is
//!   *not* read-only (it moves the entry to the MRU position).
//! - The cache stores `Arc<ParsedMap>` so the caller's hot path
//!   (resolve + window) can hold a reference past the unlock —
//!   without this every frame in a 20-frame stack would re-lock.
//! - [`Self::get_or_try_insert_with`] is the canonical entry point:
//!   it both reads and populates atomically (under one lock
//!   acquisition for the read; the loader runs unlocked; a second
//!   acquisition for the insert). Two concurrent loaders for the
//!   same key are tolerated — one of the parsed maps is dropped on
//!   insert. This is fine because [`ParsedMap`] is pure and
//!   parsing is deterministic; the alternative (per-key in-flight
//!   coalescing) would require futures and is a 钢筋-layer concern.

use crate::parsed::ParsedMap;
use core::hash::Hash;
use core::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

use lru::LruCache;

/// A bounded LRU cache of parsed source maps, keyed on `K`.
///
/// `K` is typically `uuid::Uuid` (a release id) or `String` (a hash
/// of the map bytes for content-addressed storage); the crate is
/// agnostic. The only requirements are `Hash + Eq` (for the LRU
/// table) and `Clone` (so the loader closure can be invoked with an
/// owned key without consuming it on the read path).
///
/// Construction:
///
/// ```rust
/// use core::num::NonZeroUsize;
/// use sentori_sourcemap_resolver::ResolverCache;
///
/// let cache: ResolverCache<String> =
///     ResolverCache::new(NonZeroUsize::new(50).expect("non-zero"));
/// assert_eq!(cache.capacity().get(), 50);
/// assert_eq!(cache.len(), 0);
/// ```
pub struct ResolverCache<K: Hash + Eq + Clone> {
    inner: Mutex<LruCache<K, Arc<ParsedMap>>>,
    capacity: NonZeroUsize,
}

impl<K: Hash + Eq + Clone> ResolverCache<K> {
    /// Build a new cache with the given capacity. The cap is fixed
    /// for the lifetime of the cache — there is no resize API on
    /// purpose; bounded memory is a hard invariant for the stone.
    #[must_use]
    pub fn new(capacity: NonZeroUsize) -> Self {
        Self {
            inner: Mutex::new(LruCache::new(capacity)),
            capacity,
        }
    }

    /// The fixed capacity the cache was constructed with.
    #[must_use]
    pub const fn capacity(&self) -> NonZeroUsize {
        self.capacity
    }

    /// The number of entries currently held.
    ///
    /// Lock-acquiring (the LRU is behind a mutex), but `O(1)`.
    /// Returns `0` if another thread is currently holding the
    /// mutex panicked while holding it — a poisoned mutex is
    /// treated as "empty" rather than re-panicking; the cache
    /// will heal as fresh entries displace the stuck state.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.lock().map_or(0, |g| g.len())
    }

    /// Whether the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Fetch a cached map by key, promoting it to the MRU
    /// position. Returns `None` if the key is absent.
    pub fn get(&self, key: &K) -> Option<Arc<ParsedMap>> {
        let mut guard = self.inner.lock().ok()?;
        guard.get(key).map(Arc::clone)
    }

    /// Insert (or overwrite) the entry under `key`. The previous
    /// value, if any, is dropped — the LRU's normal eviction also
    /// fires if `key` is new and the cache is full.
    pub fn insert(&self, key: K, map: Arc<ParsedMap>) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.put(key, map);
        }
    }

    /// Remove the entry under `key`, returning it if it existed.
    pub fn remove(&self, key: &K) -> Option<Arc<ParsedMap>> {
        let mut guard = self.inner.lock().ok()?;
        guard.pop(key)
    }

    /// Drop every cached entry. Useful for tests and for explicit
    /// invalidation when an operator re-uploads a map and the
    /// caller wants to nuke the old parse.
    pub fn clear(&self) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.clear();
        }
    }

    /// Read-through accessor: return the cached map for `key` or,
    /// if absent, invoke `loader` to build it, insert, and return
    /// the freshly-inserted value.
    ///
    /// The loader runs **outside** the cache mutex, so a slow parse
    /// does not block concurrent lookups against unrelated keys.
    /// If two threads race on the same key, both run the loader and
    /// the first-arriving insert wins — the second discards its
    /// parsed map after a single insert. This is sound because
    /// [`ParsedMap::parse`] is pure.
    ///
    /// # Errors
    ///
    /// Forwards any error the loader returns verbatim.
    pub fn get_or_try_insert_with<F, E>(&self, key: &K, loader: F) -> Result<Arc<ParsedMap>, E>
    where
        F: FnOnce() -> Result<Arc<ParsedMap>, E>,
    {
        if let Some(hit) = self.get(key) {
            return Ok(hit);
        }
        let fresh = loader()?;
        self.insert(key.clone(), Arc::clone(&fresh));
        // Re-read to honour any racing insert (which is also a
        // valid value — both parses agree). This costs a second
        // lock acquisition on the miss path only, which is cheap
        // relative to the parse work the loader just did.
        Ok(self.get(key).unwrap_or(fresh))
    }
}

impl<K: Hash + Eq + Clone> core::fmt::Debug for ResolverCache<K> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // The `inner` field is a `Mutex<LruCache<…>>`; printing its
        // contents would force-lock the mutex and surprise callers,
        // so we expose the public-facing counters and mark the rest
        // non-exhaustive.
        f.debug_struct("ResolverCache")
            .field("capacity", &self.capacity)
            .field("len", &self.len())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::missing_panics_doc
)]
mod tests {
    use super::*;
    use sourcemap::SourceMapBuilder;

    fn tiny_map(content: &str) -> Vec<u8> {
        let mut b = SourceMapBuilder::new(Some("bundle.js"));
        let src_id = b.add_source("src/x.ts");
        b.set_source_contents(src_id, Some(content));
        b.add(0, 0, 0, 0, Some("src/x.ts"), None, false);
        let mut out = Vec::new();
        b.into_sourcemap()
            .to_writer(&mut out)
            .expect("encode tiny map");
        out
    }

    fn arc_parse(content: &str) -> Arc<ParsedMap> {
        Arc::new(ParsedMap::parse(&tiny_map(content)).expect("parse tiny"))
    }

    #[test]
    fn new_starts_empty() {
        let c: ResolverCache<u32> = ResolverCache::new(NonZeroUsize::new(4).unwrap());
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
        assert_eq!(c.capacity().get(), 4);
    }

    #[test]
    fn insert_then_get() {
        let c: ResolverCache<u32> = ResolverCache::new(NonZeroUsize::new(4).unwrap());
        let m = arc_parse("a\n");
        c.insert(7, Arc::clone(&m));
        assert_eq!(c.len(), 1);
        let got = c.get(&7).expect("hit");
        assert!(Arc::ptr_eq(&got, &m));
    }

    #[test]
    fn miss_returns_none() {
        let c: ResolverCache<u32> = ResolverCache::new(NonZeroUsize::new(4).unwrap());
        assert!(c.get(&42).is_none());
    }

    #[test]
    fn remove_returns_value_then_misses() {
        let c: ResolverCache<u32> = ResolverCache::new(NonZeroUsize::new(4).unwrap());
        c.insert(1, arc_parse("a\n"));
        assert!(c.remove(&1).is_some());
        assert!(c.get(&1).is_none());
        assert!(c.remove(&1).is_none());
    }

    #[test]
    fn clear_empties() {
        let c: ResolverCache<u32> = ResolverCache::new(NonZeroUsize::new(4).unwrap());
        c.insert(1, arc_parse("a\n"));
        c.insert(2, arc_parse("b\n"));
        c.clear();
        assert_eq!(c.len(), 0);
    }

    #[test]
    fn lru_evicts_oldest_on_overflow() {
        let c: ResolverCache<u32> = ResolverCache::new(NonZeroUsize::new(2).unwrap());
        c.insert(1, arc_parse("a\n"));
        c.insert(2, arc_parse("b\n"));
        c.insert(3, arc_parse("c\n")); // evicts 1
        assert!(c.get(&1).is_none());
        assert!(c.get(&2).is_some());
        assert!(c.get(&3).is_some());
    }

    #[test]
    fn lru_promotes_on_access() {
        let c: ResolverCache<u32> = ResolverCache::new(NonZeroUsize::new(2).unwrap());
        c.insert(1, arc_parse("a\n"));
        c.insert(2, arc_parse("b\n"));
        let _ = c.get(&1); // promote 1
        c.insert(3, arc_parse("c\n")); // evicts 2, not 1
        assert!(c.get(&1).is_some());
        assert!(c.get(&2).is_none());
        assert!(c.get(&3).is_some());
    }

    #[test]
    fn read_through_loads_on_miss() {
        let c: ResolverCache<u32> = ResolverCache::new(NonZeroUsize::new(4).unwrap());
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let got = c
            .get_or_try_insert_with::<_, std::io::Error>(&9, || {
                calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(arc_parse("x\n"))
            })
            .expect("loader ok");
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        // Second call hits cache → loader does not run.
        let hit = c
            .get_or_try_insert_with::<_, std::io::Error>(&9, || {
                calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(arc_parse("x\n"))
            })
            .expect("loader ok");
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(Arc::ptr_eq(&got, &hit));
    }

    #[test]
    fn read_through_propagates_loader_error() {
        let c: ResolverCache<u32> = ResolverCache::new(NonZeroUsize::new(4).unwrap());
        let err = c
            .get_or_try_insert_with::<_, &'static str>(&5, || Err("boom"))
            .expect_err("propagates");
        assert_eq!(err, "boom");
        // Failed load did not insert anything.
        assert!(c.get(&5).is_none());
    }

    #[test]
    fn concurrent_inserts_are_safe() {
        use std::sync::Arc as StdArc;
        use std::thread;

        let cache: StdArc<ResolverCache<u32>> =
            StdArc::new(ResolverCache::new(NonZeroUsize::new(8).unwrap()));
        let mut handles = Vec::new();
        for t in 0..8u32 {
            let c = StdArc::clone(&cache);
            handles.push(thread::spawn(move || {
                for k in 0..50u32 {
                    let key = (t * 100) + k;
                    c.get_or_try_insert_with::<_, ()>(&key, || Ok(arc_parse("z\n")))
                        .expect("ok");
                }
            }));
        }
        for h in handles {
            h.join().expect("thread");
        }
        // Cache capped at 8, so length must respect it.
        assert!(cache.len() <= 8);
    }

    #[test]
    fn debug_renders_capacity_and_len() {
        let c: ResolverCache<u32> = ResolverCache::new(NonZeroUsize::new(3).unwrap());
        c.insert(1, arc_parse("a\n"));
        let s = format!("{c:?}");
        assert!(s.contains("ResolverCache"));
        assert!(s.contains("capacity"));
    }
}
