//! `AttachmentStore` enum — env-driven choice between in-memory
//! (default, dev) and local filesystem (production / docker volume).
//!
//! Resolves `SENTORI_ATTACHMENT_STORE` at boot:
//! - unset / "memory" — `MemoryBlobStore` (data lost on restart)
//! - "fs:/path/to/dir" — `LocalFsBlobStore` mounted at that path
//!   (typically a docker volume mounted at /data/blobs)
//!
//! Replays (`ReplayStore<S>`) keep their own typed backend
//! separately — swapping that to the enum is a follow-up.

use std::sync::Arc;

use sentori_attachment_store::{
    BlobError, BlobHash, BlobResult, BlobStore, LocalFsBlobStore, MemoryBlobStore,
};
use tracing::info;

/// Wire-protocol-stable wrapper that dispatches to the configured
/// backend at runtime.
#[derive(Clone)]
pub enum AttachmentStore {
    Memory(Arc<MemoryBlobStore>),
    Fs(Arc<LocalFsBlobStore>),
}

impl AttachmentStore {
    /// Read env at boot and construct accordingly.
    pub async fn from_env() -> BlobResult<Self> {
        let raw = std::env::var("SENTORI_ATTACHMENT_STORE").unwrap_or_default();
        if let Some(path) = raw.strip_prefix("fs:") {
            info!(path, "attachment store: local filesystem");
            let s = LocalFsBlobStore::new(path).await?;
            Ok(Self::Fs(Arc::new(s)))
        } else {
            info!("attachment store: in-memory (dev / ephemeral)");
            Ok(Self::Memory(Arc::new(MemoryBlobStore::new())))
        }
    }

    pub async fn put(&self, bytes: &[u8]) -> BlobResult<BlobHash> {
        match self {
            Self::Memory(s) => s.put(bytes).await,
            Self::Fs(s) => s.put(bytes).await,
        }
    }

    /// Read a blob back by its content hash.
    ///
    /// The wrapper exposed only `put` until now, which meant every
    /// attachment the SDK uploaded — screenshots, view trees, state
    /// snapshots, log tails, session trails, replay recordings — was
    /// write-only: stored, then unreachable. The crash detail view is
    /// built on reading these back.
    pub async fn get(&self, hash: &BlobHash) -> BlobResult<Vec<u8>> {
        match self {
            Self::Memory(s) => s.get(hash).await,
            Self::Fs(s) => s.get(hash).await,
        }
    }
}

// Suppress unused-warning when only one variant is reached at runtime.
#[allow(dead_code)]
fn _ensure_blob_error_used(_: BlobError) {}
