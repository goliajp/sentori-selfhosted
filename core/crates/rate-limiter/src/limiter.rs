//! [`Limiter`]: composes a backend + clock + policy into a
//! one-call API.
//!
//! Generic over both `B: RateBackend` and `C: Clock` so callers
//! choose their storage and time source at construction; the
//! limiter itself is a thin wrapper holding the (policy, backend,
//! clock) triple.

use crate::backend::RateBackend;
use crate::clock::Clock;
use crate::policy::{Policy, Verdict};

/// One-call rate-limit checker.
///
/// Typical use:
///
/// ```rust
/// use core::time::Duration;
/// use sentori_rate_limiter::{Limiter, MemoryBackend, Policy, SystemClock};
///
/// let policy = Policy::new(10, Duration::from_secs(60)).expect("policy");
/// let limiter = Limiter::new(MemoryBackend::new(), SystemClock, policy);
///
/// match limiter.check("user-42") {
///     v if v.is_allowed() => { /* serve the request */ }
///     v => {
///         // v is Limited { retry_after: ... }
///         let _ = v;
///     }
/// }
/// ```
#[derive(Debug)]
pub struct Limiter<B: RateBackend, C: Clock> {
    backend: B,
    clock: C,
    policy: Policy,
}

impl<B: RateBackend, C: Clock> Limiter<B, C> {
    /// Build a limiter.
    pub const fn new(backend: B, clock: C, policy: Policy) -> Self {
        Self {
            backend,
            clock,
            policy,
        }
    }

    /// Check whether `key` is within the policy and consume a
    /// slot if so. Single atomic call against the backend.
    pub fn check(&self, key: &str) -> Verdict {
        self.backend
            .check_and_consume(key, self.policy, self.clock.now())
    }

    /// Forget all state for `key`. Useful when a user
    /// authenticates: the per-IP login limit should reset for
    /// the IP that just succeeded.
    pub fn reset(&self, key: &str) {
        self.backend.reset(key);
    }

    /// Drop every entry from every key.
    pub fn clear(&self) {
        self.backend.clear();
    }

    /// The policy this limiter enforces.
    #[must_use]
    pub const fn policy(&self) -> Policy {
        self.policy
    }

    /// Borrow the backend — useful for observability hooks
    /// (e.g. report `MemoryBackend::approx_memory_bytes()`).
    pub const fn backend(&self) -> &B {
        &self.backend
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
    use crate::clock::MockClock;
    use crate::memory::MemoryBackend;
    use core::time::Duration;
    use std::time::Instant;

    fn limiter(max: u32, window_secs: u64) -> (Limiter<MemoryBackend, MockClock>, Instant) {
        let anchor = Instant::now();
        let limiter = Limiter::new(
            MemoryBackend::new(),
            MockClock::new(anchor),
            Policy::new(max, Duration::from_secs(window_secs)).expect("policy"),
        );
        (limiter, anchor)
    }

    #[test]
    fn allows_first_max_then_limits() {
        let (l, _) = limiter(3, 60);
        for _ in 0..3 {
            assert!(l.check("k").is_allowed());
        }
        assert!(l.check("k").is_limited());
    }

    #[test]
    fn shared_clock_pattern_for_time_driving_tests() {
        use std::sync::Arc;
        // Recommended pattern: callers wrap the clock in an Arc
        // and pass clones — one to the limiter (lib.rs provides
        // `impl<C: Clock> Clock for Arc<C>`), one kept for
        // advancing in the test body.
        let anchor = Instant::now();
        let clock = Arc::new(MockClock::new(anchor));
        let limiter = Limiter::new(
            MemoryBackend::new(),
            Arc::clone(&clock),
            Policy::new(2, Duration::from_secs(10)).expect("policy"),
        );
        assert!(limiter.check("k").is_allowed());
        assert!(limiter.check("k").is_allowed());
        assert!(limiter.check("k").is_limited());
        clock.advance(Duration::from_secs(11));
        assert!(limiter.check("k").is_allowed());
    }

    #[test]
    fn reset_clears_one_key() {
        let (l, _) = limiter(1, 60);
        l.check("a");
        l.check("b");
        l.reset("a");
        assert!(l.check("a").is_allowed());
        assert!(l.check("b").is_limited());
    }

    #[test]
    fn clear_drops_everything() {
        let (l, _) = limiter(1, 60);
        l.check("a");
        l.check("b");
        l.clear();
        assert!(l.check("a").is_allowed());
        assert!(l.check("b").is_allowed());
    }

    #[test]
    fn policy_accessor_round_trips() {
        let (l, _) = limiter(7, 90);
        assert_eq!(l.policy().max(), 7);
        assert_eq!(l.policy().window(), Duration::from_secs(90));
    }
}
