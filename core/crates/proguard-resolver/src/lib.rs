//! # `sentori-proguard-resolver` — Android (ProGuard / R8) mapping resolver
//!
//! Stone-tier crate (per cement-stone methodology) for deobfuscating
//! Android stack frames back to their original class / method /
//! file / line, including the R8 inline-expansion chain.
//!
//! ## What this crate does (and does not)
//!
//! - **Does** — accept raw mapping bytes (UTF-8 text — what a
//!   `mapping.txt` file on disk is), parse them into a
//!   [`ParsedMapping`], answer `obfuscated → original` lookups
//!   for both class names and `(class, method, line)` triples.
//!   [`ResolverCache`] keeps the hot mappings resident under a
//!   tunable LRU bound.
//! - **Does not** — fetch bytes over the network, read them from
//!   disk, look up "which mapping for which release / debug_id"
//!   out of a database, or rewrite specific frame struct types.
//!   Those couplings live in the 钢筋 layer (`event-pipeline` /
//!   `attachment-store`).
//! - **Does not** — handle native (DWARF) or JavaScript
//!   (sourcemap) inputs. Each has its own dedicated stone.
//!
//! ## Inline-call chain convention
//!
//! [`ParsedMapping::resolve_method`] returns a `Vec<Frame>` ordered
//! **innermost-first**: index 0 is the deepest-inlined call, the
//! outermost (the actual method the synthetic line sits in) has
//! `is_inlined = false`. Matches `sentori-dwarf-resolver`'s
//! convention so the 钢筋 layer can render JVM and native frames
//! through the same code path.
//!
//! ## Quick start
//!
//! ```rust,no_run
//! use core::num::NonZeroUsize;
//! use std::sync::Arc;
//! use sentori_proguard_resolver::{ParseError, ParsedMapping, ResolverCache};
//!
//! # fn demo(mapping_bytes: Vec<u8>) -> Result<(), Box<dyn std::error::Error>> {
//! let cache: ResolverCache<String> =
//!     ResolverCache::new(NonZeroUsize::new(8).expect("non-zero cap"));
//!
//! let key = "release-1.0.0".to_string();
//! let mapping = cache.get_or_try_insert_with(&key, || {
//!     Ok::<_, ParseError>(Arc::new(ParsedMapping::parse(mapping_bytes)?))
//! })?;
//!
//! let frames = mapping.resolve_method("a.b.c", "a", 42)?;
//! for f in &frames {
//!     println!("{}.{} at {}:{}{}",
//!         f.class, f.method,
//!         f.file.as_deref().unwrap_or("<unknown>"),
//!         f.line.unwrap_or(0),
//!         if f.is_inlined { " (inlined)" } else { "" });
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Concurrency model
//!
//! [`ParsedMapping`] is `Send + Sync` and meant to be shared via
//! `Arc`; an internal `Mutex` around the upstream
//! `proguard::ProguardMapper` (which carries `RefCell` internal
//! caches like `addr2line::Context`) makes the type thread-safe.
//! The cache uses `std::sync::Mutex<LruCache>` for the same
//! reason `sourcemap-resolver` / `dwarf-resolver` do — LRU `get`
//! mutates (promotes) the entry so `RwLock` would be a category
//! error.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
// Crate docs interleave narrative with tech identifiers ("R8",
// "JVM", "URLs") the doc_markdown heuristic mis-flags as un-
// codified. Prose readability over satisfying the lint.
#![allow(clippy::doc_markdown)]
// `multiple_crate_versions` fires on transitive dep versions we
// don't control.
#![allow(clippy::multiple_crate_versions)]
// `redundant_pub_crate` flags `pub(crate)` items inside private
// modules; we use it deliberately so a future `pub use` doesn't
// accidentally widen visibility beyond `pub(crate)`.
#![allow(clippy::redundant_pub_crate)]
// `future_not_send` fires on ouroboros' generated `*_async`
// builder methods — futures across an `await` of the self-
// referencing closure. We never use the async builders.
#![allow(clippy::future_not_send)]

mod cache;
mod error;
mod frame;
mod mapping;

#[cfg(test)]
mod test_fixtures;

pub use cache::ResolverCache;
pub use error::{ParseError, ParseResult, ResolveError, ResolveResult};
pub use frame::Frame;
pub use mapping::ParsedMapping;
