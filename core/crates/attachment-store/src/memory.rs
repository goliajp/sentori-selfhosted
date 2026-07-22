//! [`MemoryBlobStore`] — in-process `HashMap` impl for tests + dev fallback.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use crate::error::BlobResult;
use crate::hash::BlobHash;
use crate::store::BlobStore;

/// HashMap-backed blob store. Lives entirely in process memory;
/// data is lost when the value is dropped.
///
/// Use cases:
/// - Unit + integration tests that need a `BlobStore` impl
///   without a tempdir.
/// - Dev fallback when neither `SENTORI_ATTACHMENT_DIR` nor a
///   cloud-storage env block are configured — the binary still
///   runs, attachments just don't persist across restarts.
///   (The wiring layer chooses; this crate just provides the
///   primitive.)
///
/// Thread-safety: an inner `Mutex` serialises access. A
/// poisoned lock recovers the inner guard rather than
/// propagating the poison — the HashMap state itself is
/// invariant-clean (we only mutate inside `lock()`-guarded
/// regions).
pub struct MemoryBlobStore {
    blobs: Mutex<HashMap<[u8; 32], Vec<u8>>>,
}

impl MemoryBlobStore {
    /// Empty store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            blobs: Mutex::new(HashMap::new()),
        }
    }

    /// Number of distinct blobs currently held. Cheap O(1) on
    /// the underlying HashMap.
    #[must_use]
    pub fn count(&self) -> usize {
        lock(&self.blobs).len()
    }

    /// Sum of all blob payload bytes. Iterates all values; O(N).
    #[must_use]
    pub fn total_bytes(&self) -> usize {
        lock(&self.blobs).values().map(Vec::len).sum()
    }
}

impl Default for MemoryBlobStore {
    fn default() -> Self {
        Self::new()
    }
}

impl BlobStore for MemoryBlobStore {
    async fn put(&self, bytes: &[u8]) -> BlobResult<BlobHash> {
        let hash = BlobHash::of(bytes);
        {
            let mut map = lock(&self.blobs);
            // Idempotent: re-inserting the same bytes is a no-op.
            map.entry(*hash.as_bytes())
                .or_insert_with(|| bytes.to_vec());
        }
        Ok(hash)
    }

    async fn get(&self, hash: &BlobHash) -> BlobResult<Vec<u8>> {
        let out = {
            let map = lock(&self.blobs);
            map.get(hash.as_bytes()).cloned()
        };
        out.ok_or(crate::error::BlobError::NotFound)
    }

    async fn exists(&self, hash: &BlobHash) -> BlobResult<bool> {
        let out = {
            let map = lock(&self.blobs);
            map.contains_key(hash.as_bytes())
        };
        Ok(out)
    }

    async fn len(&self, hash: &BlobHash) -> BlobResult<Option<u64>> {
        let out = {
            let map = lock(&self.blobs);
            map.get(hash.as_bytes()).map(|v| v.len() as u64)
        };
        Ok(out)
    }

    async fn delete(&self, hash: &BlobHash) -> BlobResult<()> {
        {
            let mut map = lock(&self.blobs);
            map.remove(hash.as_bytes());
        }
        Ok(())
    }
}

/// Acquire the inner mutex, recovering from poison.
///
/// Poison would mean a previous holder panicked mid-mutation,
/// but `MemoryBlobStore`'s mutations are atomic HashMap
/// operations — there is no "half-written" state to leak. We
/// keep going.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}
