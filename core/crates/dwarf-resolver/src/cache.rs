//! Bounded LRU cache around [`DwarfModule`].
//!
//! Same shape as `sentori-sourcemap-resolver::ResolverCache` (intentional
//! — when a third consumer arrives we'll extract them to a shared
//! `arc-lru` stone). Parsing a 5-50 MB iOS dSYM costs single-digit
//! milliseconds per symbolication burst; doing it for every frame
//! in a 20-frame stack would dominate the ingest budget. This cache
//! keeps the hot modules resident under a tunable LRU bound.
//!
//! ## Design notes (deltas from the sourcemap-resolver cache)
//!
//! - **Different value type, identical surface.** This cache stores
//!   `Arc<DwarfModule>`. The LRU + Mutex pattern is identical; the
//!   key type is generic on `K: Hash + Eq + Clone`.
//! - **Larger entries.** A parsed sourcemap is ~10 KB; a parsed
//!   dSYM is ~5-50 MB. Callers should size capacity proportionally
//!   smaller — a cap of 8-16 is typically right for native
//!   symbolication, vs the 50-100 a JS resolver might use.
//! - **Same race-tolerant read-through semantics.** Two threads
//!   loading the same dSYM both parse; the first insert wins.
//!   Sound because [`DwarfModule::from_bytes`] is pure.

use core::hash::Hash;
use core::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

use lru::LruCache;

use crate::module::DwarfModule;

/// A bounded LRU cache of parsed DWARF modules, keyed on `K`.
///
/// `K` is typically a `(debug_id, arch)` tuple or a content-hash of
/// the slice bytes; this crate is agnostic. The only requirements
/// are `Hash + Eq` (LRU table) and `Clone` (loader callback).
///
/// Construction:
///
/// ```rust
/// use core::num::NonZeroUsize;
/// use sentori_dwarf_resolver::ResolverCache;
///
/// let cache: ResolverCache<String> =
///     ResolverCache::new(NonZeroUsize::new(8).expect("non-zero"));
/// assert_eq!(cache.capacity().get(), 8);
/// assert_eq!(cache.len(), 0);
/// ```
pub struct ResolverCache<K: Hash + Eq + Clone> {
    inner: Mutex<LruCache<K, Arc<DwarfModule>>>,
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

    /// The number of entries currently held. A poisoned mutex is
    /// treated as "empty" rather than re-panicking; the cache
    /// heals as fresh entries displace the stuck state.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.lock().map_or(0, |g| g.len())
    }

    /// Whether the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Fetch a cached module by key, promoting it to the MRU
    /// position. Returns `None` if the key is absent.
    pub fn get(&self, key: &K) -> Option<Arc<DwarfModule>> {
        let mut guard = self.inner.lock().ok()?;
        guard.get(key).map(Arc::clone)
    }

    /// Insert (or overwrite) the entry under `key`.
    pub fn insert(&self, key: K, module: Arc<DwarfModule>) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.put(key, module);
        }
    }

    /// Remove the entry under `key`, returning it if it existed.
    pub fn remove(&self, key: &K) -> Option<Arc<DwarfModule>> {
        let mut guard = self.inner.lock().ok()?;
        guard.pop(key)
    }

    /// Drop every cached entry.
    pub fn clear(&self) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.clear();
        }
    }

    /// Read-through accessor: return the cached module for `key`
    /// or, if absent, invoke `loader` to build it, insert, and
    /// return the freshly-inserted value.
    ///
    /// Loader runs **outside** the cache mutex; concurrent lookups
    /// against unrelated keys are not blocked. Two threads racing
    /// on the same key both load — sound because
    /// [`DwarfModule::from_bytes`] is pure.
    ///
    /// # Errors
    ///
    /// Forwards any error the loader returns verbatim.
    pub fn get_or_try_insert_with<F, E>(&self, key: &K, loader: F) -> Result<Arc<DwarfModule>, E>
    where
        F: FnOnce() -> Result<Arc<DwarfModule>, E>,
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
    use crate::test_fixtures::synthetic_macho_with_dwarf;

    fn arc_module() -> Arc<DwarfModule> {
        let fx = synthetic_macho_with_dwarf();
        Arc::new(DwarfModule::from_bytes(fx.bytes).expect("parse"))
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
        let m = arc_module();
        c.insert(7, Arc::clone(&m));
        let got = c.get(&7).expect("hit");
        assert!(Arc::ptr_eq(&got, &m));
    }

    #[test]
    fn lru_evicts_oldest_on_overflow() {
        let c: ResolverCache<u32> = ResolverCache::new(NonZeroUsize::new(2).unwrap());
        let m = arc_module();
        c.insert(1, Arc::clone(&m));
        c.insert(2, Arc::clone(&m));
        c.insert(3, Arc::clone(&m)); // evicts 1
        assert!(c.get(&1).is_none());
        assert!(c.get(&2).is_some());
        assert!(c.get(&3).is_some());
    }

    #[test]
    fn remove_returns_value_then_misses() {
        let c: ResolverCache<u32> = ResolverCache::new(NonZeroUsize::new(4).unwrap());
        c.insert(1, arc_module());
        assert!(c.remove(&1).is_some());
        assert!(c.get(&1).is_none());
        assert!(c.remove(&1).is_none());
    }

    #[test]
    fn clear_empties() {
        let c: ResolverCache<u32> = ResolverCache::new(NonZeroUsize::new(4).unwrap());
        c.insert(1, arc_module());
        c.insert(2, arc_module());
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
                Ok(arc_module())
            })
            .expect("ok");
        let _ = c
            .get_or_try_insert_with::<_, std::io::Error>(&9, || {
                calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(arc_module())
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
        c.insert(1, arc_module());
        let s = format!("{c:?}");
        assert!(s.contains("ResolverCache"));
        assert!(s.contains("capacity"));
    }
}
