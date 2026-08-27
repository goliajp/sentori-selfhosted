//! Property tests for [`sentori_event_ringbuffer::Ring`].
//!
//! Covers FIFO invariants, capacity bounds, drop accounting, and the
//! relationship between `push` outcomes and the drop counter.

// As with the stress test, the proptest harness uses small loop
// counters whose `usize`/`u32`/`u64` casts are deliberate and bound
// by the strategy widths. Bulk-allow truncation lints here to avoid
// noise.
#![allow(
    missing_docs,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_panics_doc,
    clippy::cast_possible_truncation,
    clippy::cast_lossless,
    // `is_empty == (len == 0)` is the invariant we're asserting, so
    // the lint would defeat the purpose.
    clippy::len_zero
)]

use proptest::prelude::*;
use sentori_event_ringbuffer::{PushOutcome, Ring};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(192))]

    /// Without overflow, the buffer drains in FIFO order, drops 0.
    #[test]
    fn fifo_without_overflow(
        capacity in 1usize..=64,
        items in proptest::collection::vec(any::<u32>(), 0..=64),
    ) {
        prop_assume!(items.len() <= capacity);
        let ring: Ring<u32> = Ring::with_capacity(capacity).unwrap();
        for v in &items {
            prop_assert_eq!(ring.push(*v), PushOutcome::Inserted);
        }
        prop_assert_eq!(ring.dropped_count(), 0);
        prop_assert_eq!(ring.len(), items.len());

        let mut drained = Vec::with_capacity(items.len());
        while let Some(v) = ring.pop() {
            drained.push(v);
        }
        prop_assert_eq!(drained, items);
    }

    /// With overflow, the surviving suffix == the last `capacity`
    /// items pushed, drops == `pushed - capacity`.
    #[test]
    fn drop_oldest_suffix_invariant(
        capacity in 1usize..=32,
        items in proptest::collection::vec(any::<u32>(), 1..=128),
    ) {
        let ring: Ring<u32> = Ring::with_capacity(capacity).unwrap();
        for v in &items {
            ring.push(*v);
        }

        let n = items.len();
        let expected_drops = n.saturating_sub(capacity) as u64;
        prop_assert_eq!(ring.dropped_count(), expected_drops);

        let suffix_start = n - n.min(capacity);
        let expected: Vec<u32> = items[suffix_start..].to_vec();
        let mut drained = Vec::with_capacity(expected.len());
        while let Some(v) = ring.pop() {
            drained.push(v);
        }
        prop_assert_eq!(drained, expected);
    }

    /// `len() <= capacity()` always, and `is_full()` iff `len() ==
    /// capacity()`.
    #[test]
    fn len_and_full_consistency(
        capacity in 1usize..=16,
        ops in proptest::collection::vec(any::<bool>(), 0..=128),
    ) {
        let ring: Ring<u32> = Ring::with_capacity(capacity).unwrap();
        let mut next: u32 = 0;
        for is_push in ops {
            if is_push {
                ring.push(next);
                next = next.wrapping_add(1);
            } else {
                let _ = ring.pop();
            }
            prop_assert!(ring.len() <= ring.capacity());
            prop_assert_eq!(ring.is_full(), ring.len() == ring.capacity());
            prop_assert_eq!(ring.is_empty(), ring.len() == 0);
        }
    }

    /// The first `capacity` pushes are `Inserted`; everything beyond
    /// is `InsertedAfterEviction` (single-threaded — no race-loss
    /// path).
    #[test]
    fn outcome_breakdown_single_thread(
        capacity in 1usize..=16,
        extra in 0usize..=32,
    ) {
        let ring: Ring<u32> = Ring::with_capacity(capacity).unwrap();
        for i in 0..capacity {
            prop_assert_eq!(
                ring.push(u32::try_from(i).unwrap()),
                PushOutcome::Inserted,
            );
        }
        for i in 0..extra {
            prop_assert_eq!(
                ring.push(u32::try_from(capacity + i).unwrap()),
                PushOutcome::InsertedAfterEviction,
            );
        }
        prop_assert_eq!(ring.dropped_count(), extra as u64);
    }

    /// Clones share state — pushing through any clone is visible to
    /// any other clone.
    #[test]
    fn clone_sharing_invariant(
        capacity in 1usize..=32,
        items in proptest::collection::vec(any::<u32>(), 0..=32),
    ) {
        let ring: Ring<u32> = Ring::with_capacity(capacity).unwrap();
        let a = ring.clone();
        let b = ring.clone();
        for v in &items {
            a.push(*v);
        }
        prop_assert_eq!(b.len(), ring.len());
        prop_assert_eq!(b.dropped_count(), ring.dropped_count());
    }
}
