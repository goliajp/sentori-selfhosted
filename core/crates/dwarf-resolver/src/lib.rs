//! # `sentori-dwarf-resolver` — native (DWARF / Mach-O) symbolicator
//!
//! Stone-tier crate (per cement-stone methodology) for resolving
//! native iOS / macOS / Catalyst stack frames back to original
//! `(function, file:line[:column])` chains, including the full
//! inlined-call hierarchy DWARF preserves.
//!
//! ## What this crate does (and does not)
//!
//! - **Does** — accept raw Mach-O bytes (the inner Mach-O of a
//!   dSYM bundle, or a non-stripped executable / object file),
//!   parse the DWARF sections, and answer
//!   `static_offset → Vec<Frame>` queries. Companion
//!   [`MachoSlicer`] extracts a single-arch slice from a fat
//!   (universal) Mach-O. [`ResolverCache`] keeps the hot modules
//!   resident under a tunable LRU bound.
//! - **Does not** — fetch bytes over the network, read them from
//!   disk, look up "which module for which `debug_id`" out of a
//!   database, unwrap a dSYM bundle directory, or rewrite specific
//!   frame struct types. Those couplings live in the 钢筋 layer
//!   (`event-pipeline` / `attachment-store`).
//! - **Does not** — handle Hermes (RN bytecode), Proguard (Android
//!   minification), or sourcemap (JS) inputs. Each has its own
//!   dedicated stone.
//!
//! ## Coordinate convention
//!
//! - **Offset — `u64`, static.** The offset passed to
//!   [`DwarfModule::resolve`] is `PC - image_base` — the address
//!   the SDK observed at crash time minus the load address the
//!   binary was placed at by the kernel (ASLR slide). Callers
//!   typically read both from the iOS native crash handler and
//!   subtract before calling us.
//! - **Inlined chain — innermost-first `Vec<Frame>`.** Index 0
//!   is the deepest-inlined call; the last frame's `is_inlined`
//!   flag is `false` (it is the lexically-innermost real function
//!   the PC lives in).
//!
//! ## Quick start
//!
//! ```rust,no_run
//! use core::num::NonZeroUsize;
//! use std::sync::Arc;
//! use sentori_dwarf_resolver::{Arch, DwarfModule, MachoSlicer, ResolverCache};
//!
//! # fn demo(fat_bytes: Vec<u8>) -> Result<(), Box<dyn std::error::Error>> {
//! let cache: ResolverCache<(String, Arch)> =
//!     ResolverCache::new(NonZeroUsize::new(8).expect("non-zero cap"));
//!
//! let key = ("BD93D1D5-...".to_string(), Arch::Arm64);
//! let module = cache.get_or_try_insert_with(&key, || {
//!     let slice = MachoSlicer::slice(&fat_bytes, Arch::Arm64)?;
//!     Ok::<_, Box<dyn std::error::Error>>(
//!         Arc::new(DwarfModule::from_bytes(slice)?),
//!     )
//! })?;
//!
//! let frames = module.resolve(0x1234_5000)?;
//! for f in &frames {
//!     println!("{}: {}:{}{}",
//!         f.function.as_deref().unwrap_or("<unknown>"),
//!         f.file.as_deref().unwrap_or("<no file>"),
//!         f.line.unwrap_or(0),
//!         if f.is_inlined { " (inlined)" } else { "" });
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Concurrency model
//!
//! [`DwarfModule`] is `Send + Sync` and meant to be shared via
//! `Arc`; the inner [`addr2line::Context`] is lock-free for the
//! read path. The cache uses `std::sync::Mutex<LruCache>` for the
//! same reason `sourcemap-resolver` does — LRU `get` mutates
//! (promotes) the entry so `RwLock` would be a category error.
//! Race-tolerant: two threads loading the same module both parse;
//! the first insert wins, the second's parse is dropped. Sound
//! because [`DwarfModule::from_bytes`] is pure.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
// Crate docs interleave narrative with tech identifiers ("PC",
// "ASLR", "URLs") the doc_markdown heuristic mis-flags as un-
// codified. Prose readability over satisfying the lint.
#![allow(clippy::doc_markdown)]
// `multiple_crate_versions` fires when `object` / `gimli` pull a
// `hashbrown` major different from the workspace tip — upstream,
// not ours.
#![allow(clippy::multiple_crate_versions)]
// `redundant_pub_crate` flags `pub(crate)` items inside private
// modules. We use `pub(crate)` deliberately so adding a `pub use`
// later doesn't accidentally widen visibility past `pub(crate)`
// for cross-module helpers — the lint's preference for plain `pub`
// inverts that safety property.
#![allow(clippy::redundant_pub_crate)]
// `future_not_send` fires on ouroboros' generated `*_async` builder
// methods — futures across an `await` of the self-referencing
// closure. We never use the async builders (every `from_bytes` is
// sync), so the lint is a false positive in our usage pattern.
#![allow(clippy::future_not_send)]

mod arch;
mod cache;
mod error;
mod frame;
mod macho;
mod module;

#[cfg(test)]
mod test_fixtures;

pub use arch::Arch;
pub use cache::ResolverCache;
pub use error::{ParseError, ParseResult, ResolveError, ResolveResult, SliceError, SliceResult};
pub use frame::Frame;
pub use macho::MachoSlicer;
pub use module::DwarfModule;
