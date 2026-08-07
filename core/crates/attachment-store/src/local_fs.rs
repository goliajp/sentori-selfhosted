//! [`LocalFsBlobStore`] — directory tree with 2-char prefix fanout.

use std::path::{Path, PathBuf};

use crate::error::{BlobError, BlobResult};
use crate::hash::BlobHash;
use crate::store::BlobStore;

/// Local-filesystem-backed blob store.
///
/// Layout under `root`:
///
/// ```text
/// <root>/
/// ├── ab/
/// │   ├── cdef0123…ff.bin
/// │   └── …
/// ├── cd/
/// │   └── …
/// └── …
/// ```
///
/// Each blob lives at `<root>/<first-2-hex-chars>/<remaining-62-chars>.bin`.
/// 256 first-level dirs is the standard balance — see Sentry,
/// git, IPFS. Deeper fanout becomes worth it once a single first-
/// level dir exceeds ~10K entries (≈ 2.5 M total blobs); v0.1
/// won't approach that on self-hosted.
///
/// Writes go through a `.tmp` sibling + rename so a crashed
/// write never leaves a half-written blob visible to readers.
/// (The legacy `attachments.rs` shipped this pattern after a
/// production incident where a SIGTERM during write left a
/// truncated PNG served from `get`.)
pub struct LocalFsBlobStore {
    root: PathBuf,
}

impl LocalFsBlobStore {
    /// Construct over `root`. Creates the directory if absent.
    ///
    /// # Errors
    ///
    /// [`BlobError::Backend`] if the directory cannot be created
    /// (no permission, no space, etc.).
    pub async fn new(root: impl Into<PathBuf>) -> BlobResult<Self> {
        let root = root.into();
        tokio::fs::create_dir_all(&root).await.map_err(|e| {
            BlobError::backend(format!("failed to create root dir {}", root.display()), e)
        })?;
        Ok(Self { root })
    }

    /// Construct without ensuring the root exists. Useful when
    /// the caller has already created / mounted the directory
    /// (Docker volume, k8s persistent volume, etc.).
    ///
    /// Mistakes here surface at first `put` as a backend error.
    #[must_use]
    pub fn new_unchecked(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Borrow the configured root directory.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn dir_for(&self, hash: &BlobHash) -> PathBuf {
        let hex = hash.to_hex();
        self.root.join(&hex[..2])
    }

    fn path_for(&self, hash: &BlobHash) -> PathBuf {
        let hex = hash.to_hex();
        // hex is exactly 64 chars; split at 2.
        self.root.join(&hex[..2]).join(format!("{}.bin", &hex[2..]))
    }
}

impl BlobStore for LocalFsBlobStore {
    async fn put(&self, bytes: &[u8]) -> BlobResult<BlobHash> {
        let hash = BlobHash::of(bytes);
        let dir = self.dir_for(&hash);
        tokio::fs::create_dir_all(&dir).await.map_err(|e| {
            BlobError::backend(format!("failed to create fanout dir {}", dir.display()), e)
        })?;

        let final_path = self.path_for(&hash);
        // Skip the write if it's already on disk (idempotency).
        if tokio::fs::try_exists(&final_path).await.unwrap_or(false) {
            return Ok(hash);
        }

        // tmp -> rename so crashed writes don't leave a partial
        // blob visible to get(). Use the hash + ".tmp" suffix so
        // concurrent writes of the same blob don't collide.
        let tmp_path = final_path.with_extension("bin.tmp");
        tokio::fs::write(&tmp_path, bytes)
            .await
            .map_err(|e| BlobError::backend(format!("write tmp {}", tmp_path.display()), e))?;
        if let Err(e) = tokio::fs::rename(&tmp_path, &final_path).await {
            // Best-effort cleanup of the tmp.
            let _ = tokio::fs::remove_file(&tmp_path).await;
            return Err(BlobError::backend(
                format!("rename {} -> {}", tmp_path.display(), final_path.display()),
                e,
            ));
        }
        Ok(hash)
    }

    async fn get(&self, hash: &BlobHash) -> BlobResult<Vec<u8>> {
        let path = self.path_for(hash);
        match tokio::fs::read(&path).await {
            Ok(bytes) => Ok(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(BlobError::NotFound),
            Err(e) => Err(BlobError::backend(format!("read {}", path.display()), e)),
        }
    }

    async fn list(&self) -> BlobResult<Vec<(BlobHash, std::time::SystemTime)>> {
        let mut out = Vec::new();
        let mut top = match tokio::fs::read_dir(&self.root).await {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
            Err(e) => {
                return Err(BlobError::backend(
                    format!("read_dir {}", self.root.display()),
                    e,
                ));
            }
        };
        while let Some(dir) = top
            .next_entry()
            .await
            .map_err(|e| BlobError::backend("read_dir entry", e))?
        {
            let prefix = dir.file_name();
            let Some(prefix) = prefix.to_str().map(str::to_owned) else {
                continue;
            };
            // fanout dirs are exactly the 2-hex-char prefixes.
            if prefix.len() != 2 || !prefix.chars().all(|c| c.is_ascii_hexdigit()) {
                continue;
            }
            let Ok(mut inner) = tokio::fs::read_dir(dir.path()).await else {
                continue;
            };
            while let Some(f) = inner
                .next_entry()
                .await
                .map_err(|e| BlobError::backend("read_dir entry", e))?
            {
                let name = f.file_name();
                let Some(rest) = name.to_str().and_then(|n| n.strip_suffix(".bin")) else {
                    // .bin.tmp (crashed write) and anything else is
                    // not a blob; the mtime guard in the GC caller is
                    // what keeps in-flight tmps safe, not this skip.
                    continue;
                };
                let Ok(hash) = format!("{prefix}{rest}").parse::<BlobHash>() else {
                    continue;
                };
                let Ok(md) = f.metadata().await else { continue };
                let mtime = md.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                out.push((hash, mtime));
            }
        }
        Ok(out)
    }

    async fn exists(&self, hash: &BlobHash) -> BlobResult<bool> {
        let path = self.path_for(hash);
        tokio::fs::try_exists(&path)
            .await
            .map_err(|e| BlobError::backend(format!("stat {}", path.display()), e))
    }

    async fn len(&self, hash: &BlobHash) -> BlobResult<Option<u64>> {
        let path = self.path_for(hash);
        match tokio::fs::metadata(&path).await {
            Ok(md) => Ok(Some(md.len())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(BlobError::backend(format!("stat {}", path.display()), e)),
        }
    }

    async fn delete(&self, hash: &BlobHash) -> BlobResult<()> {
        let path = self.path_for(hash);
        match tokio::fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(BlobError::backend(format!("delete {}", path.display()), e)),
        }
    }
}
