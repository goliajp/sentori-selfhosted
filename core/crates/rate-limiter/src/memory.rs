//! In-memory sliding-window-log [`RateBackend`].
//!
//! Per key, holds a `VecDeque<Instant>` of timestamps for the
//! requests inside the current rolling window. On each
//! `check_and_consume`:
//!
//! 1. Drop entries older than `now - policy.window()` from the
//!    front of the deque.
//! 2. If `deque.len() < policy.max()`, push `now`, return
//!    `Allowed { remaining = max - new_len }`.
//! 3. Otherwise return `Limited { retry_after = oldest + window -
//!    now }` — the earliest moment the oldest entry will fall out
//!    of the window.
//!
//! Sliding-window log is **precise**: the rate over any sliding
//! window of `policy.window()` length is guaranteed ≤ `policy.max()`.
//! The downside is O(max) memory per key. For the v0.1 dashboard's
//! ~30 req/min per token, that's 30 timestamps × 16 bytes = 480
//! bytes / key — trivial. For the K-tier Valkey backend handling
//! 1000s req/min, the implementation will trade precision for
//! O(1) memory via the "fixed-window-counter with weighted
//! carryover" technique; this is why the trait abstracts the
//! storage shape.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::Instant;

use crate::backend::RateBackend;
use crate::policy::{Policy, Verdict};

/// Sliding-window-log in-memory backend.
///
/// Internal `Mutex<HashMap<…>>`: every `check_and_consume`
/// acquires the lock briefly. For 1000s of concurrent callers
/// against distinct keys this can become a contention point;
/// the Valkey backend in the K-tier is the alternative when
/// scale demands it.
#[derive(Default, Debug)]
pub struct MemoryBackend {
    inner: Mutex<HashMap<String, VecDeque<Instant>>>,
}

impl MemoryBackend {
    /// Build an empty backend.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Compute how many bytes the backend is currently using
    /// (approximate). Useful for observability / capacity
    /// planning.
    #[must_use]
    pub fn approx_memory_bytes(&self) -> usize {
        let Ok(g) = self.inner.lock() else {
            return 0;
        };
        let mut total = 0usize;
        for (key, deque) in g.iter() {
            total += key.capacity();
            // `VecDeque<Instant>` stores Instants contiguously;
            // each is 16 bytes on every Tier-1 target.
            total += deque
                .capacity()
                .saturating_mul(core::mem::size_of::<Instant>());
        }
        total
    }
}

impl RateBackend for MemoryBackend {
    fn check_and_consume(&self, key: &str, policy: Policy, now: Instant) -> Verdict {
        let Ok(mut g) = self.inner.lock() else {
            // Poisoned mutex: fail open (allow) rather than wedge
            // the limiter forever. The legacy `server/src/rate_limit
            // .rs` did the same on Valkey-unavailable.
            return Verdict::Allowed {
                remaining: policy.max(),
            };
        };
        let deque = g.entry(key.to_owned()).or_default();

        // 1. Prune anything older than (now - window).
        let cutoff = now.checked_sub(policy.window());
        if let Some(c) = cutoff {
            while let Some(&front) = deque.front() {
                if front < c {
                    deque.pop_front();
                } else {
                    break;
                }
            }
        }
        // If `checked_sub` underflowed (now is earlier than
        // window — only possible on a non-monotonic clock or
        // an empty deque shortly after process start), leave
        // the deque alone; the size check below still works.

        // 2. Check + insert.
        let max = policy.max();
        if u32::try_from(deque.len()).unwrap_or(u32::MAX) < max {
            deque.push_back(now);
            let new_len = u32::try_from(deque.len()).unwrap_or(u32::MAX);
            Verdict::Allowed {
                remaining: max - new_len,
            }
        } else {
            // 3. Limited — retry_after = oldest + window - now.
            let oldest = deque.front().copied().unwrap_or(now);
            let earliest_free = oldest + policy.window();
            let retry_after = earliest_free
                .checked_duration_since(now)
                .unwrap_or_default();
            Verdict::Limited { retry_after }
        }
    }

    fn reset(&self, key: &str) {
        if let Ok(mut g) = self.inner.lock() {
            g.remove(key);
        }
    }

    fn clear(&self) {
        if let Ok(mut g) = self.inner.lock() {
            g.clear();
        }
    }

    fn approx_key_count(&self) -> usize {
        self.inner.lock().map_or(0, |g| g.len())
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::missing_panics_doc
)]
mod tests {
    use super::*;
    use core::time::Duration;

    fn p(max: u32, window_secs: u64) -> Policy {
        Policy::new(max, Duration::from_secs(window_secs)).expect("policy")
    }

    #[test]
    fn first_call_is_allowed() {
        let b = MemoryBackend::new();
        let v = b.check_and_consume("k", p(3, 60), Instant::now());
        assert!(matches!(v, Verdict::Allowed { remaining: 2 }));
    }

    #[test]
    fn allows_up_to_max() {
        let b = MemoryBackend::new();
        let now = Instant::now();
        let policy = p(3, 60);
        for _ in 0..3 {
            assert!(b.check_and_consume("k", policy, now).is_allowed());
        }
        let v = b.check_and_consume("k", policy, now);
        assert!(v.is_limited());
    }

    #[test]
    fn limited_retry_after_is_oldest_plus_window() {
        let b = MemoryBackend::new();
        let now = Instant::now();
        let policy = p(2, 10);
        b.check_and_consume("k", policy, now); // oldest
        b.check_and_consume("k", policy, now + Duration::from_secs(3));
        // Try after 5 seconds — oldest = now, window = 10s, so
        // we should wait (now + 10s) - (now + 5s) = 5s.
        let v = b.check_and_consume("k", policy, now + Duration::from_secs(5));
        match v {
            Verdict::Limited { retry_after } => {
                assert_eq!(retry_after, Duration::from_secs(5));
            }
            Verdict::Allowed { .. } => panic!("expected Limited, got Allowed"),
        }
    }

    #[test]
    fn window_slides_after_stale_entries_drop() {
        let b = MemoryBackend::new();
        let t0 = Instant::now();
        let policy = p(2, 10);
        // Burn the 2 slots at t0.
        b.check_and_consume("k", policy, t0);
        b.check_and_consume("k", policy, t0);
        // At t0+11s the first two are stale → fresh budget.
        let v = b.check_and_consume("k", policy, t0 + Duration::from_secs(11));
        assert!(matches!(v, Verdict::Allowed { remaining: 1 }));
    }

    #[test]
    fn distinct_keys_have_independent_buckets() {
        let b = MemoryBackend::new();
        let now = Instant::now();
        let policy = p(1, 60);
        assert!(b.check_and_consume("a", policy, now).is_allowed());
        assert!(b.check_and_consume("b", policy, now).is_allowed());
        // Neither should be limited yet — both at 1/1 in
        // independent buckets.
        assert!(b.check_and_consume("a", policy, now).is_limited());
        assert!(b.check_and_consume("b", policy, now).is_limited());
    }

    #[test]
    fn reset_clears_one_key() {
        let b = MemoryBackend::new();
        let now = Instant::now();
        let policy = p(1, 60);
        b.check_and_consume("a", policy, now);
        b.check_and_consume("b", policy, now);
        b.reset("a");
        assert!(b.check_and_consume("a", policy, now).is_allowed());
        assert!(b.check_and_consume("b", policy, now).is_limited());
    }

    #[test]
    fn clear_drops_everything() {
        let b = MemoryBackend::new();
        let now = Instant::now();
        let policy = p(1, 60);
        b.check_and_consume("a", policy, now);
        b.check_and_consume("b", policy, now);
        assert_eq!(b.approx_key_count(), 2);
        b.clear();
        assert_eq!(b.approx_key_count(), 0);
    }

    #[test]
    fn approx_memory_bytes_reports_non_zero_with_entries() {
        let b = MemoryBackend::new();
        let now = Instant::now();
        b.check_and_consume("some-key", p(5, 60), now);
        assert!(b.approx_memory_bytes() > 0);
    }

    #[test]
    fn approx_memory_bytes_is_zero_when_empty() {
        let b = MemoryBackend::new();
        assert_eq!(b.approx_memory_bytes(), 0);
    }

    #[test]
    fn limited_verdict_carries_zero_retry_after_when_window_already_elapsed() {
        // Construct a path where checked_duration_since underflows.
        // Manually inject a "limit hit" state, then check far in
        // the future — oldest + window < now, so retry_after = 0.
        let b = MemoryBackend::new();
        let t0 = Instant::now();
        let policy = p(1, 5);
        b.check_and_consume("k", policy, t0);
        // At t0+100s, the oldest entry would have fallen out
        // already → pruning step removes it before the limit
        // check → result is `Allowed`, not `Limited`. This
        // implicitly tests that we don't synthesise a "Limited
        // with retry=0" result.
        let v = b.check_and_consume("k", policy, t0 + Duration::from_secs(100));
        assert!(v.is_allowed());
    }
}
