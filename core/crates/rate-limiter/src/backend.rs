//! Storage abstraction for the limiter.
//!
//! Stones can't take a network dep, so the Valkey / Redis
//! backend lives in the 钢筋 `auth-session` crate above. This
//! trait is the seam; in the stone we ship [`MemoryBackend`]
//! (sliding-window log, single-process). In v0.2 a
//! `ValkeyBackend` implementing the same trait will let callers
//! swap without touching `Limiter` call sites.

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
    /// internally count (e.g. a Valkey backend that wouldn't
    /// `KEYS *` to count — that's an O(N) Redis no-go).
    fn approx_key_count(&self) -> usize;
}
