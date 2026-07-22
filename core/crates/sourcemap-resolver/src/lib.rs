//! # `sentori-sourcemap-resolver` — JS Source Map V3 resolver + LRU cache
//!
//! Stone-tier crate (per cement-stone methodology) for symbolicating
//! minified JavaScript stack frames back to original-source
//! positions. Wraps the upstream `sourcemap` crate (the canonical
//! Rust parser, also used by Sentry, Datadog, and Mozilla) with a
//! typed result struct, a windowed-source helper, and a bounded LRU
//! cache so a 20-frame stack does not re-parse the same megabyte of
//! `.map` JSON twenty times.
//!
//! ## What this crate does (and does not)
//!
//! - **Does** — accept raw `.map` bytes (or an index map's split
//!   sections), parse them into an immutable [`ParsedMap`], answer
//!   `(line, column) → original (file, line, column, function)`
//!   queries, and emit `±N`-line source-context windows from the
//!   bundler's `sourcesContent` if it was embedded. A
//!   [`ResolverCache`] keeps the hot maps resident under a tunable
//!   LRU bound.
//! - **Does not** — fetch maps over the network, read them from
//!   disk, look up "which map for which release" out of a database,
//!   or mutate any specific frame type. Those couplings live in the
//!   钢筋 layer (`event-pipeline`); a stone has no business knowing
//!   about HTTP or Postgres.
//! - **Does not** — handle Hermes (React Native bytecode) source
//!   maps. Hermes maps share the JSON envelope but use a different
//!   lookup primitive (`bytecode_offset` rather than `line:column`)
//!   and a dedicated stone alongside `dwarf-resolver` will cover
//!   them. This crate refuses Hermes input at parse time via
//!   [`error::ParseError::UnsupportedFormat`] rather than silently
//!   degrading to wrong answers.
//!
//! ## Coordinate convention
//!
//! Inputs and outputs follow what every JS engine reports for stack
//! frames:
//!
//! - **Line — 1-indexed.** `line == 0` is invalid and returns
//!   `None` immediately. The very first line of a file is `1`.
//! - **Column — 0-indexed.** Columns are byte offsets into the
//!   minified line; `0` is the start of the line.
//!
//! The Source Map V3 spec stores both as 0-indexed internally;
//! [`ParsedMap::resolve`] does the `±1` correction so callers can
//! plug raw JS engine values straight in without the off-by-one
//! ceremony every Sentry-style integrator gets wrong at least once.
//!
//! ## Quick start
//!
//! ```rust
//! use core::num::NonZeroUsize;
//! use std::sync::Arc;
//! use sentori_sourcemap_resolver::{ParsedMap, ResolverCache};
//!
//! # fn demo(map_bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
//! let cache: ResolverCache<String> =
//!     ResolverCache::new(NonZeroUsize::new(50).expect("non-zero cap"));
//!
//! let key = "release-v1.2.3".to_string();
//! let map = cache.get_or_try_insert_with(&key, || {
//!     Ok::<_, sentori_sourcemap_resolver::ParseError>(
//!         Arc::new(ParsedMap::parse(map_bytes)?),
//!     )
//! })?;
//!
//! if let Some(r) = map.resolve(42, 1280) {
//!     println!("{}:{}:{} ({})",
//!         r.file.as_deref().unwrap_or("<anonymous>"),
//!         r.line, r.column,
//!         r.function.as_deref().unwrap_or("<anonymous>"));
//!     if let Some(win) = map.source_window(r.src_id, (r.line - 1) as usize, 3) {
//!         println!("> {}", win.at);
//!     }
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Concurrency model
//!
//! [`ParsedMap`] is `Send + Sync` and meant to be shared via `Arc`;
//! the underlying `sourcemap::SourceMap` is immutable. The cache
//! uses a `std::sync::Mutex<LruCache>` internally — LRU `get` is
//! not actually a read operation (it promotes the entry), so an
//! `RwLock` would be a category error. Two threads racing on the
//! same key will both run the loader; the second insert wins and
//! the loser's parse is dropped. This is sound because parsing is
//! pure and deterministic, and avoids dragging in `futures` for
//! per-key in-flight coalescing that belongs in the 钢筋 layer.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
// Module documentation interleaves narrative paragraphs with tech
// terms (`HTTP`, `URLs`, `RN`) the doc_markdown heuristic mis-flags
// as un-codified identifiers. Keeping the prose readable matters
// more than satisfying the lint.
#![allow(clippy::doc_markdown)]
// `multiple_crate_versions` fires because `sourcemap`'s dep graph
// pulls a different `bitflags` major than the workspace tip; that's
// upstream, not ours.
#![allow(clippy::multiple_crate_versions)]

mod cache;
mod error;
mod parsed;

pub use cache::ResolverCache;
pub use error::{ParseError, ParseResult};
pub use parsed::{ParsedMap, Resolution, SourceWindow};
