//! Bounded LRU cache around [`ParsedMapping`].
//!
//! Same shape as `sentori-sourcemap-resolver::ResolverCache` and
//! `sentori-dwarf-resolver::ResolverCache` (intentional — when a
//! fourth consumer arrives we'll extract them to a shared
//! `arc-lru` stone).
//!
//! Mapping files range from ~100 KB (small library) to ~10 MB
//! (full obfuscated Android app). The parsing cost is dominated by
//! the upstream's class-index build; doing it per frame in a
//! 20-frame stack would be 20× wasteful, so a small LRU (cap ≈ 8)
//! pays for itself the moment a project sees more than one event
//! against the same mapping.

use core::hash::Hash;
use core::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

use lru::LruCache;

use crate::mapping::ParsedMapping;

/// A bounded LRU cache of parsed ProGuard / R8 mappings, keyed
/// on `K`.
///
/// `K` is typically `uuid::Uuid` (the R8 `pg_map_id`) or a
/// `(project_id, release_name)` tuple; this crate is agnostic.
/// Requirements are `Hash + Eq` (for the LRU table) and `Clone`
/// (loader callback).
///
/// Construction:
///
/// ```rust
/// use core::num::NonZeroUsize;
/// use sentori_proguard_resolver::ResolverCache;
///
/// let cache: ResolverCache<String> =
///     ResolverCache::new(NonZeroUsize::new(8).expect("non-zero"));
/// assert_eq!(cache.capacity().get(), 8);
/// assert_eq!(cache.len(), 0);
/// ```
pub struct ResolverCache<K: Hash + Eq + Clone> {
    inner: Mutex<LruCache<K, Arc<ParsedMapping>>>,
    capacity: NonZeroUsize,
}

impl<K: Hash + Eq + Clone> ResolverCache<K> {
    /// Build a new cache with the given capacity. The cap is fixed
    /// for the lifetime of the cache — bounded memory is a hard
    /// invariant for the stone.
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

    /// The number of entries currently held. A poisoned mutex
    /// is treated as "empty" rather than re-panicking.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.lock().map_or(0, |g| g.len())
    }

    /// Whether the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Fetch a cached mapping by key, promoting it to the MRU
    /// position. Returns `None` if the key is absent.
    pub fn get(&self, key: &K) -> Option<Arc<ParsedMapping>> {
        let mut guard = self.inner.lock().ok()?;
        guard.get(key).map(Arc::clone)
    }

    /// Insert (or overwrite) the entry under `key`.
    pub fn insert(&self, key: K, mapping: Arc<ParsedMapping>) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.put(key, mapping);
        }
    }

    /// Remove the entry under `key`, returning it if it existed.
    pub fn remove(&self, key: &K) -> Option<Arc<ParsedMapping>> {
        let mut guard = self.inner.lock().ok()?;
        guard.pop(key)
    }

    /// Drop every cached entry.
    pub fn clear(&self) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.clear();
        }
    }

    /// Read-through accessor: return the cached mapping for `key`
    /// or, if absent, invoke `loader` to build it, insert, and
    /// return the freshly-inserted value.
    ///
    /// Loader runs **outside** the cache mutex; concurrent lookups
    /// against unrelated keys are not blocked. Two threads racing
    /// on the same key both load — sound because
    /// [`ParsedMapping::parse`] is pure.
    ///
    /// # Errors
    ///
    /// Forwards any error the loader returns verbatim.
    pub fn get_or_try_insert_with<F, E>(&self, key: &K, loader: F) -> Result<Arc<ParsedMapping>, E>
    where
        F: FnOnce() -> Result<Arc<ParsedMapping>, E>,
    {
        if let Some(hit) = self.get(key) {
            return Ok(hit);
        }
        let fresh = loader()?;
        self.insert(key.clone(), Arc::clone(&fresh));
        Ok(self.get(key).unwrap_or(fresh))
    }
}

impl<K: Hash + Eq + Clone> core::fmt::Debug for ResolverCache<K> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // `inner` is `Mutex<LruCache>`; printing its contents would
        // force-lock and surprise callers. Expose counters only.
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
    use crate::test_fixtures::SIMPLE_MAPPING;

    fn arc_mapping() -> Arc<ParsedMapping> {
        Arc::new(ParsedMapping::parse(SIMPLE_MAPPING.as_bytes().to_vec()).expect("parse"))
    }

    #[test]
    fn new_starts_empty() {
        let c: ResolverCache<u32> = ResolverCache::new(NonZeroUsize::new(4).unwrap());
        assert!(c.is_empty());
        assert_eq!(c.capacity().get(), 4);
    }

    #[test]
    fn insert_then_get() {
        let c: ResolverCache<u32> = ResolverCache::new(NonZeroUsize::new(4).unwrap());
        let m = arc_mapping();
        c.insert(7, Arc::clone(&m));
        let got = c.get(&7).expect("hit");
        assert!(Arc::ptr_eq(&got, &m));
    }

    #[test]
    fn lru_evicts_oldest_on_overflow() {
        let c: ResolverCache<u32> = ResolverCache::new(NonZeroUsize::new(2).unwrap());
        let m = arc_mapping();
        c.insert(1, Arc::clone(&m));
        c.insert(2, Arc::clone(&m));
        c.insert(3, Arc::clone(&m));
        assert!(c.get(&1).is_none());
        assert!(c.get(&2).is_some());
        assert!(c.get(&3).is_some());
    }

    #[test]
    fn remove_returns_value_then_misses() {
        let c: ResolverCache<u32> = ResolverCache::new(NonZeroUsize::new(4).unwrap());
        c.insert(1, arc_mapping());
        assert!(c.remove(&1).is_some());
        assert!(c.get(&1).is_none());
        assert!(c.remove(&1).is_none());
    }

    #[test]
    fn clear_empties() {
        let c: ResolverCache<u32> = ResolverCache::new(NonZeroUsize::new(4).unwrap());
        c.insert(1, arc_mapping());
        c.insert(2, arc_mapping());
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn read_through_loads_on_miss_then_hits() {
        let c: ResolverCache<u32> = ResolverCache::new(NonZeroUsize::new(4).unwrap());
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let _ = c
            .get_or_try_insert_with::<_, std::io::Error>(&9, || {
                calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(arc_mapping())
            })
            .expect("ok");
        let _ = c
            .get_or_try_insert_with::<_, std::io::Error>(&9, || {
                calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(arc_mapping())
            })
            .expect("ok");
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn read_through_propagates_loader_error() {
        let c: ResolverCache<u32> = ResolverCache::new(NonZeroUsize::new(4).unwrap());
        let err = c
            .get_or_try_insert_with::<_, &'static str>(&5, || Err("boom"))
            .expect_err("propagates");
        assert_eq!(err, "boom");
        assert!(c.get(&5).is_none());
    }

    #[test]
    fn debug_renders_capacity_and_len() {
        let c: ResolverCache<u32> = ResolverCache::new(NonZeroUsize::new(3).unwrap());
        c.insert(1, arc_mapping());
        let s = format!("{c:?}");
        assert!(s.contains("ResolverCache"));
        assert!(s.contains("capacity"));
    }
}
