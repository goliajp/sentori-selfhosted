//! [`BlobStore`] trait — the public surface every backend implements.

use std::future::Future;

use crate::error::BlobResult;
use crate::hash::BlobHash;

/// Content-addressed blob store.
///
/// Implementations:
/// - [`crate::LocalFsBlobStore`] — fanned-out directory tree.
/// - [`crate::MemoryBlobStore`] — `HashMap` (tests / dev).
///
/// Future:
/// - S3 / GCS / Azure (per K3 follow-up issue tracked in
///   v0.1-execution-plan).
///
/// ## Async-fn-in-trait
///
/// Uses the stable Rust 1.75+ "async fn in trait" feature.
/// Trait objects (`dyn BlobStore`) are NOT directly supported
/// — the compiler requires an opaque ABI wrapper for that. K3
/// ships the trait in generic form only; if a consumer needs
/// `dyn` dispatch later, wrap-on-demand with the trait-object-
/// safe `async-trait` shim or a hand-rolled object-safe facade.
///
/// For now wire your consumer over the concrete impl type
/// (`IngestPipeline<S: BlobStore>` etc. rather than holding a
/// `Box<dyn BlobStore>`).
pub trait BlobStore: Send + Sync {
    /// Store `bytes`. Returns the hash that addresses them.
    ///
    /// Idempotent — calling `put` twice with the same payload
    /// yields the same hash and is a no-op the second time
    /// (backend may skip the write entirely).
    ///
    /// # Errors
    ///
    /// [`crate::BlobError::Backend`] on backend I/O failure.
    fn put(&self, bytes: &[u8]) -> impl Future<Output = BlobResult<BlobHash>> + Send;

    /// Fetch the blob with the given hash.
    ///
    /// # Errors
    ///
    /// - [`crate::BlobError::NotFound`] if no blob with this
    ///   hash exists.
    /// - [`crate::BlobError::Backend`] on backend I/O.
    fn get(&self, hash: &BlobHash) -> impl Future<Output = BlobResult<Vec<u8>>> + Send;

    /// Like [`Self::get`] but re-hash the payload after the
    /// read and verify it matches the requested hash. Pays
    /// O(blob_size) extra hashing — use on every read if you
    /// don't trust your storage backend, or on a sampled basis
    /// from a janitor.
    ///
    /// # Errors
    ///
    /// - [`crate::BlobError::NotFound`] if no blob exists.
    /// - [`crate::BlobError::HashMismatch`] on corruption /
    ///   tampering.
    /// - [`crate::BlobError::Backend`] on backend I/O.
    fn get_verified(&self, hash: &BlobHash) -> impl Future<Output = BlobResult<Vec<u8>>> + Send {
        async move {
            let bytes = self.get(hash).await?;
            let actual = BlobHash::of(&bytes);
            if &actual == hash {
                Ok(bytes)
            } else {
                Err(crate::error::BlobError::HashMismatch)
            }
        }
    }

    /// True if a blob with this hash exists.
    ///
    /// Implementations should not load the blob's bytes for
    /// this check.
    ///
    /// # Errors
    ///
    /// [`crate::BlobError::Backend`] on backend I/O.
    fn exists(&self, hash: &BlobHash) -> impl Future<Output = BlobResult<bool>> + Send;

    /// Return the blob's length in bytes, or `None` if it
    /// doesn't exist.
    ///
    /// # Errors
    ///
    /// [`crate::BlobError::Backend`] on backend I/O.
    fn len(&self, hash: &BlobHash) -> impl Future<Output = BlobResult<Option<u64>>> + Send;

    /// Delete the blob. Idempotent (silent no-op if absent).
    ///
    /// # Errors
    ///
    /// [`crate::BlobError::Backend`] on backend I/O.
    fn delete(&self, hash: &BlobHash) -> impl Future<Output = BlobResult<()>> + Send;
}
