//! Property tests for the sliding-window invariants.
//!
//! Three core invariants:
//!
//! 1. **No over-budget burst.** No matter how requests are
//!    distributed in time, the number allowed in any rolling
//!    window of `policy.window()` length never exceeds
//!    `policy.max()`.
//! 2. **Eventual recovery.** After advancing past `policy.window()`
//!    from the last call, the bucket is fully refreshed.
//! 3. **Distinct keys are independent.** Saturating key A's
//!    bucket has no effect on key B.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_panics_doc,
    clippy::too_many_lines,
    missing_docs
)]

use core::time::Duration;
use std::sync::Arc;
use std::time::Instant;

use proptest::prelude::*;

use sentori_rate_limiter::{Clock, Limiter, MemoryBackend, MockClock, Policy};

fn build_limiter(
    max: u32,
    window_secs: u64,
) -> (Limiter<MemoryBackend, Arc<MockClock>>, Arc<MockClock>) {
    let anchor = Instant::now();
    let clock = Arc::new(MockClock::new(anchor));
    let limiter = Limiter::new(
        MemoryBackend::new(),
        Arc::clone(&clock),
        Policy::new(max, Duration::from_secs(window_secs)).expect("policy"),
    );
    (limiter, clock)
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 128,
        .. ProptestConfig::default()
    })]

    /// **Invariant 1**: Sum of allowed verdicts inside any
    /// sliding window of `window_secs` ≤ `max`.
    ///
    /// Strategy: generate a sequence of (`gap_ms`,) per request,
    /// then walk the sequence advancing the mock clock by gap
    /// between each call. After every call, replay the recent
    /// call log to count how many were allowed inside the
    /// `[now - window, now]` interval and assert ≤ `max`.
    #[test]
    fn never_exceeds_limit_in_any_window(
        max in 1u32..16,
        window_secs in 1u64..30,
        gaps in prop::collection::vec(0u64..3000, 1..30),
    ) {
        let (limiter, clock) = build_limiter(max, window_secs);
        let window = Duration::from_secs(window_secs);
        let mut log: Vec<(Instant, bool)> = Vec::new();

        for gap_ms in &gaps {
            clock.advance(Duration::from_millis(*gap_ms));
            let now = clock.now();
            let allowed = limiter.check("k").is_allowed();
            log.push((now, allowed));

            // Count allowed entries inside [now - window, now].
            let cutoff = now.checked_sub(window);
            let allowed_in_window: usize = log
                .iter()
                .filter(|&&(t, a)| {
                    a && cutoff.is_none_or(|c| t >= c)
                })
                .count();
            prop_assert!(
                allowed_in_window <= max as usize,
                "allowed {allowed_in_window} > max {max} in window {window_secs}s",
            );
        }
    }

    /// **Invariant 2**: After advancing well past the window, a
    /// previously-saturated bucket fully refreshes.
    #[test]
    fn full_recovery_after_window(
        max in 1u32..16,
        window_secs in 1u64..10,
    ) {
        let (limiter, clock) = build_limiter(max, window_secs);
        // Saturate.
        for _ in 0..max {
            limiter.check("k");
        }
        prop_assert!(limiter.check("k").is_limited());

        // Advance past the window.
        clock.advance(Duration::from_secs(window_secs + 1));

        // Should be allowed again, max times.
        for _ in 0..max {
            prop_assert!(limiter.check("k").is_allowed());
        }
        prop_assert!(limiter.check("k").is_limited());
    }

    /// **Invariant 3**: Two keys' buckets are independent.
    #[test]
    fn distinct_keys_independent(
        max in 1u32..16,
        window_secs in 1u64..10,
        key_a in "[a-z]{3,8}",
        key_b in "[A-Z]{3,8}",
    ) {
        prop_assume!(key_a != key_b);
        let (limiter, _) = build_limiter(max, window_secs);

        // Saturate key A.
        for _ in 0..max {
            limiter.check(&key_a);
        }
        prop_assert!(limiter.check(&key_a).is_limited());

        // Key B should still be fully open.
        for _ in 0..max {
            prop_assert!(limiter.check(&key_b).is_allowed());
        }
    }

    /// **Invariant 4 (Verdict shape)**: `Allowed` payload's
    /// `remaining` is correct — equal to `max - 1 - prior_allowed`
    /// for sequential same-instant calls.
    #[test]
    fn allowed_remaining_is_max_minus_consumed(
        max in 1u32..16,
        n_calls in 1u32..8,
    ) {
        prop_assume!(n_calls <= max);
        let (limiter, _) = build_limiter(max, 60);

        for i in 0..n_calls {
            use sentori_rate_limiter::Verdict;
            match limiter.check("k") {
                Verdict::Allowed { remaining } => {
                    prop_assert_eq!(remaining, max - 1 - i);
                }
                Verdict::Limited { .. } => {
                    prop_assert!(false, "expected Allowed at iter {i}, got Limited");
                }
            }
        }
    }
}
