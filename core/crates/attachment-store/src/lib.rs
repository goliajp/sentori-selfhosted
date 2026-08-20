//! # `sentori-attachment-store` — content-addressed blob store
//!
//! Steel-tier (钢筋) crate #3. Stores opaque blobs (screenshots,
//! view-tree dumps, replay frames, sourcemap files, dSYM
//! archives, ProGuard maps, …) keyed by SHA-256 of the bytes.
//!
//! ## Why content-addressed
//!
//! Three properties fall out for free when the key IS the hash:
//!
//! 1. **Dedup across owners.** Two projects that captured the
//!    same app icon, or two replay sessions that captured an
//!    identical frame, share storage. No DB join required to
//!    notice the duplicate; the second `put` just rediscovers
//!    the existing object.
//! 2. **Tamper-evident reads.** A reader can re-hash the
//!    payload after `get` and compare to the requested hash;
//!    a backend that corrupted or swapped the blob can't fake
//!    matching content. (This crate's verify-on-read is
//!    optional via [`BlobStore::get_verified`].)
//! 3. **Trivial dump / restore.** D6 portability per
//!    product-architecture §09 says blobs must round-trip
//!    through `.sentori-dump`. With CAS the dump layer just
//!    copies `(hash, bytes)` pairs; cross-edition imports
//!    can `exists()` first and skip writes that already match.
//!
//! ## What this crate IS NOT
//!
//! Not a database. Has no per-owner index. The blob set is
//! flat — consumers (K4 ingest, K8 replay, K11 notifier) own
//! their own `(owner_id, blob_hash)` tables and decide when
//! to call [`BlobStore::delete`].
//!
//! GC is a janitor concern: walk the union of every consumer's
//! refs, compare against the store's `list` (not provided by
//! this crate's trait; janitor binaries reach for backend-
//! native enumeration — `WalkDir` on the LocalFs root, list
//! prefix on S3, etc.).
//!
//! ## Trait surface (narrow on purpose)
//!
//! ```text
//! trait BlobStore:
//!   put(bytes) → BlobHash
//!   get(hash)  → Vec<u8>
//!   get_verified(hash) → Vec<u8>  -- rehashes after read
//!   exists(hash) → bool
//!   len(hash)    → Option<u64>
//!   delete(hash) → ()
//! ```
//!
//! No streaming yet. No presigned URLs. No range reads. Add
//! when a real consumer asks; YAGNI rules the kitchen.
//!
//! ## Backends shipped
//!
//! - [`LocalFsBlobStore`] — directory tree under a root path
//!   with 2-char prefix fanout (`<root>/<hh>/<rest>.bin`).
//!   Sentry / git / IPFS all use the same layout for the same
//!   reasons (256 first-level dirs balances filesystem-perf
//!   vs `ls` usability).
//! - [`MemoryBlobStore`] — HashMap-backed. For unit tests, the
//!   trait_compliance suite, and the "no config given" dev
//!   fallback (caller wires it in when env vars aren't set).
//!
//! S3 / GCS / Azure are deferred to follow-ups (per user
//! decision 2026-06-20). The trait shape doesn't preclude
//! them; the `aws-sdk-s3` dep tree just isn't worth it for
//! v0.1 since self-hosted and saas both run local-fs.
//!
//! ## Quick start
//!
//! ```no_run
//! use sentori_attachment_store::{BlobStore, LocalFsBlobStore};
//! use std::path::PathBuf;
//!
//! # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
//! let store = LocalFsBlobStore::new(PathBuf::from("/var/lib/sentori/blobs")).await?;
//! let hash = store.put(b"some payload").await?;
//! let back = store.get(&hash).await?;
//! assert_eq!(back, b"some payload");
//! # Ok(()) }
//! ```

#![cfg_attr(docsrs, feature(doc_cfg))]
// Doc backticks: identifier-heavy prose reads cleaner without
// backticking every snake_case word.
#![allow(clippy::doc_markdown)]

mod error;
mod hash;
mod local_fs;
mod memory;
mod store;

pub use error::{BlobError, BlobResult};
pub use hash::{BLOB_HASH_BYTES, BlobHash, BlobHashParseError};
pub use local_fs::LocalFsBlobStore;
pub use memory::MemoryBlobStore;
pub use store::BlobStore;
