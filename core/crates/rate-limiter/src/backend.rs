//! Storage abstraction for the limiter.
//!
//! Stones can't take a network dep, so a network-backed impl
//! would live in a 钢筋 crate above. This trait is the seam, and
//! it currently has exactly one implementation: [`MemoryBackend`]
//! (sliding-window log, single-process). The seam is what would
//! let a cross-process backend land without touching `Limiter`
//! call sites.

use std::time::Instant;

use crate::policy::{Policy, Verdict};

/// A storage backend for the sliding-window limiter.
///
/// The trait is `Send + Sync` because the typical caller is an
/// axum middleware running on a multi-threaded executor; the
/// implementation is responsible for its own internal locking.
pub trait RateBackend: Send + Sync {
    /// Atomically: prune stale entries for `key`, check whether
    /// inserting a new entry at `now` would exceed `policy`,
    /// insert iff allowed, return the verdict.
    ///
    /// The single-call contract is important. A separate
    /// `count` then `insert` pair would race under concurrent
    /// callers (two threads both see count = limit−1, both
    /// insert, count becomes limit + 1). Implementations MUST
    /// serialise the prune-check-insert into one critical
    /// section.
    fn check_and_consume(&self, key: &str, policy: Policy, now: Instant) -> Verdict;

    /// Drop all state for `key`. Used by tests and by the
    /// caller when a user authenticates (reset the per-IP login
    /// limit for that IP, say).
    fn reset(&self, key: &str);

    /// Drop every entry from every key. Mostly useful for tests
    /// and for periodic "clear at startup" hooks.
    fn clear(&self);

    /// Approximate count of keys currently tracking state.
    /// Implementations may return 0 if the backend doesn't
    /// internally count (a network backend would not scan its
    /// whole keyspace just to answer this).
    fn approx_key_count(&self) -> usize;
}
