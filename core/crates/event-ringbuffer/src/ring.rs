//! [`Ring`] — bounded MPMC ring buffer with drop-oldest overflow.
//!
//! Thin wrapper around [`crossbeam_queue::ArrayQueue`] that adds the
//! eviction policy and observability counter described in the crate
//! module docs.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crossbeam_queue::ArrayQueue;

use crate::error::{CapacityError, RingResult};

/// Minimum capacity accepted by [`Ring::with_capacity`]. Equal to 1;
/// zero is rejected.
pub const MIN_CAPACITY: usize = 1;

/// Outcome of a single [`Ring::push`].
///
/// Exposed so the caller can react to the three distinct states the
/// ring may have been in at push time — useful both for telemetry
/// breakdowns and for tests that want to assert a particular policy
/// path triggered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PushOutcome {
    /// The item was inserted into a non-full ring. No eviction
    /// happened.
    Inserted,
    /// The ring was full; the oldest queued item was evicted and the
    /// new item took its slot. Net throughput is preserved — one
    /// older event was lost.
    InsertedAfterEviction,
    /// The new item could not be placed. This is the lose-the-race
    /// path: a concurrent producer filled the slot before this
    /// thread's retry-push could land.
    ///
    /// In single-producer workloads this variant is unreachable; it
    /// only surfaces under heavy MP contention. May correspond to
    /// either 1 or 2 underlying drops (one for the new item; an
    /// additional one if the eviction probe also evicted an older
    /// queued item that the racing producer then displaced) — see
    /// `Ring::push` source for the full breakdown.
    Dropped,
}

/// Bounded lock-free ring buffer with drop-oldest overflow and atomic
/// drop telemetry.
///
/// Construct with [`Ring::with_capacity`]. Clone is cheap — the
/// underlying queue + counter are an [`Arc`] inside, so cloned
/// handles share state and can be moved across threads / async tasks.
/// `T: Send` is required (it's MPMC) and most use-cases want
/// `T: 'static` too, but neither bound is encoded statically here —
/// they apply at the call site where the clone crosses a thread or
/// task boundary.
#[derive(Debug)]
pub struct Ring<T> {
    inner: Arc<Inner<T>>,
}

#[derive(Debug)]
struct Inner<T> {
    queue: ArrayQueue<T>,
    /// Total events dropped over the lifetime of the ring. Monotonic.
    dropped: AtomicU64,
}

impl<T> Ring<T> {
    /// Build a ring with the given fixed capacity.
    ///
    /// # Errors
    ///
    /// Returns [`CapacityError::Zero`] if `capacity == 0`. Any other
    /// non-zero value is accepted; the underlying
    /// [`ArrayQueue`] allocates `capacity * size_of::<T>()` bytes
    /// once at construction and never grows.
    pub fn with_capacity(capacity: usize) -> RingResult<Self> {
        if capacity < MIN_CAPACITY {
            return Err(CapacityError::Zero);
        }
        Ok(Self {
            inner: Arc::new(Inner {
                queue: ArrayQueue::new(capacity),
                dropped: AtomicU64::new(0),
            }),
        })
    }

    /// Try to push `item` into the ring.
    ///
    /// See [`PushOutcome`] for the three return paths. The drop
    /// counter is incremented by the exact number of items lost on
    /// this call (0, 1, or 2) — see implementation comments for the
    /// rare two-loss path under heavy contention.
    ///
    /// Lock-free; safe to call from many threads at once.
    pub fn push(&self, item: T) -> PushOutcome {
        match self.inner.queue.push(item) {
            Ok(()) => PushOutcome::Inserted,
            Err(item) => {
                // Buffer was full at the moment of `push`. Try to
                // evict the oldest item to make room.
                //
                // Both `pop()` and the retry `push()` can succeed OR
                // fail independently under MP contention — a
                // concurrent consumer might have just drained the
                // queue (pop returns None), and a concurrent
                // producer might have refilled it before our retry
                // (push returns Err). Both observations matter for
                // accurate drop accounting.
                let evicted = self.inner.queue.pop();
                let pushed = self.inner.queue.push(item);

                // Each of the four (evicted?, pushed?) combinations
                // gives a distinct (outcome, items-lost) pair: the
                // outcome the caller observes, and the number we add
                // to the drop counter for accurate accounting.
                let (outcome, lost): (PushOutcome, u64) = match (evicted.is_some(), pushed.is_ok())
                {
                    // Normal eviction path: evicted one, took
                    // its slot. One item lost.
                    (true, true) => (PushOutcome::InsertedAfterEviction, 1),
                    // Both lost: eviction freed a slot, racing
                    // producer took it before our retry, and
                    // our new item rebounds. Old + new = 2.
                    (true, false) => (PushOutcome::Dropped, 2),
                    // Race-recovered to a clean insert: consumer
                    // drained between our failed push and our
                    // eviction probe, retry pushed into a real
                    // free slot. Nothing lost.
                    (false, true) => (PushOutcome::Inserted, 0),
                    // Consumer drained, but a concurrent
                    // producer refilled before our retry — only
                    // the new item is lost.
                    (false, false) => (PushOutcome::Dropped, 1),
                };

                if lost > 0 {
                    self.inner.dropped.fetch_add(lost, Ordering::Relaxed);
                }

                outcome
            }
        }
    }

    /// Pop the oldest queued item, or [`None`] if the ring is empty.
    ///
    /// Lock-free. Safe to call from multiple consumer threads, though
    /// the project's expected use is a single drainer task.
    #[must_use]
    pub fn pop(&self) -> Option<T> {
        self.inner.queue.pop()
    }

    /// Current number of queued items.
    ///
    /// This is a momentary observation; concurrent producers /
    /// consumers can change it before the caller acts on the value.
    /// Use for back-pressure heuristics, not for decisions that
    /// must be atomic with respect to the next push or pop.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.queue.len()
    }

    /// Whether the ring currently holds zero items.
    ///
    /// Same caveat as [`Self::len`] — observation only.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.queue.is_empty()
    }

    /// Whether the ring is currently at capacity.
    ///
    /// Same caveat as [`Self::len`]; a `true` here followed by a
    /// `push` may still succeed if a concurrent consumer drained
    /// between the two calls.
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.inner.queue.is_full()
    }

    /// Fixed capacity the ring was constructed with.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.inner.queue.capacity()
    }

    /// Total number of items dropped over the lifetime of the ring.
    ///
    /// Monotonic across the ring's lifetime. Combines both
    /// [`PushOutcome::InsertedAfterEviction`] and
    /// [`PushOutcome::Dropped`] outcomes.
    #[must_use]
    pub fn dropped_count(&self) -> u64 {
        self.inner.dropped.load(Ordering::Relaxed)
    }
}

impl<T> Clone for Ring<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
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

    #[test]
    fn zero_capacity_rejected() {
        let err = Ring::<u8>::with_capacity(0).unwrap_err();
        assert_eq!(err, CapacityError::Zero);
    }

    #[test]
    fn capacity_one_accepted() {
        let ring: Ring<u8> = Ring::with_capacity(1).unwrap();
        assert_eq!(ring.capacity(), 1);
        assert!(ring.is_empty());
        assert!(!ring.is_full());
    }

    #[test]
    fn fifo_under_no_overflow() {
        let ring: Ring<u32> = Ring::with_capacity(8).unwrap();
        for i in 0..4 {
            assert_eq!(ring.push(i), PushOutcome::Inserted);
        }
        assert_eq!(ring.len(), 4);
        for i in 0..4 {
            assert_eq!(ring.pop(), Some(i));
        }
        assert_eq!(ring.pop(), None);
        assert_eq!(ring.dropped_count(), 0);
    }

    #[test]
    fn drop_oldest_on_overflow() {
        let ring: Ring<u32> = Ring::with_capacity(3).unwrap();
        assert_eq!(ring.push(1), PushOutcome::Inserted);
        assert_eq!(ring.push(2), PushOutcome::Inserted);
        assert_eq!(ring.push(3), PushOutcome::Inserted);
        // 4th push at capacity: evicts 1, inserts 4.
        assert_eq!(ring.push(4), PushOutcome::InsertedAfterEviction);
        assert_eq!(ring.len(), 3);
        assert_eq!(ring.dropped_count(), 1);
        assert_eq!(ring.pop(), Some(2));
        assert_eq!(ring.pop(), Some(3));
        assert_eq!(ring.pop(), Some(4));
        assert_eq!(ring.pop(), None);
    }

    #[test]
    fn many_overflow_pushes_increment_counter_each_time() {
        let ring: Ring<u32> = Ring::with_capacity(2).unwrap();
        for i in 0..10 {
            ring.push(i);
        }
        // First 2 are clean inserts; next 8 each evict one.
        assert_eq!(ring.dropped_count(), 8);
        // Surviving items are the last 2.
        assert_eq!(ring.pop(), Some(8));
        assert_eq!(ring.pop(), Some(9));
        assert_eq!(ring.pop(), None);
    }

    #[test]
    fn pop_on_empty_returns_none() {
        let ring: Ring<u8> = Ring::with_capacity(4).unwrap();
        assert_eq!(ring.pop(), None);
    }

    #[test]
    fn is_full_flag_tracks_state() {
        let ring: Ring<u32> = Ring::with_capacity(2).unwrap();
        assert!(!ring.is_full());
        ring.push(1);
        assert!(!ring.is_full());
        ring.push(2);
        assert!(ring.is_full());
        assert_eq!(ring.pop(), Some(1));
        assert!(!ring.is_full());
    }

    #[test]
    fn clone_shares_state() {
        let ring: Ring<u32> = Ring::with_capacity(4).unwrap();
        let cloned = ring.clone();
        ring.push(1);
        assert_eq!(cloned.len(), 1);
        assert_eq!(cloned.pop(), Some(1));
        assert_eq!(ring.len(), 0);
    }

    #[test]
    fn clone_shares_drop_counter() {
        let ring: Ring<u32> = Ring::with_capacity(1).unwrap();
        let cloned = ring.clone();
        ring.push(1);
        ring.push(2); // evicts 1
        assert_eq!(cloned.dropped_count(), 1);
    }

    #[test]
    fn debug_impl_works() {
        let ring: Ring<u32> = Ring::with_capacity(4).unwrap();
        let _ = format!("{ring:?}");
    }

    #[test]
    fn capacity_one_round_trip() {
        // Pathological smallest ring: every overflow is also an
        // immediate eviction, exercising the tightest loop.
        let ring: Ring<u32> = Ring::with_capacity(1).unwrap();
        assert_eq!(ring.push(1), PushOutcome::Inserted);
        assert_eq!(ring.push(2), PushOutcome::InsertedAfterEviction);
        assert_eq!(ring.push(3), PushOutcome::InsertedAfterEviction);
        assert_eq!(ring.dropped_count(), 2);
        assert_eq!(ring.pop(), Some(3));
        assert_eq!(ring.pop(), None);
    }

    #[test]
    fn capacity_reported_matches_construction() {
        for cap in [1, 2, 7, 16, 1024] {
            let ring: Ring<u32> = Ring::with_capacity(cap).unwrap();
            assert_eq!(ring.capacity(), cap);
        }
    }
}
