//! Concurrent stress test — many producer threads + drain consumer
//! threads racing on a small ring. Verifies the no-deadlock /
//! no-corruption invariants the lock-free design promises.
//!
//! Single end-to-end harness rather than a property test; proptest
//! shrinking on threaded code is unreliable and the cost-vs-coverage
//! tradeoff for one well-shaped scenario is better here.

// Casting lints are allowed at file scope: the constants in this
// stress test (`PRODUCERS`, `PER_PRODUCER`, `ITEMS`, etc.) are
// hand-picked to fit comfortably in `u64` and `usize` on both 32-
// and 64-bit hosts. Forcing `try_from(...).expect(...)` everywhere
// would clutter the harness without catching any real bug.
#![allow(
    missing_docs,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_panics_doc,
    clippy::cast_possible_truncation,
    clippy::cast_lossless,
    clippy::cast_sign_loss
)]

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crossbeam_utils::thread::scope;
use sentori_event_ringbuffer::Ring;

/// 4 producer threads, 2 consumer threads, small ring → high
/// contention so the eviction + lose-the-race path both fire.
#[test]
fn no_corruption_under_contention() {
    const PRODUCERS: usize = 4;
    const CONSUMERS: usize = 2;
    const PER_PRODUCER: usize = 5_000;
    const CAPACITY: usize = 32;

    let ring: Ring<u64> = Ring::with_capacity(CAPACITY).unwrap();
    let stop_consumers = Arc::new(AtomicUsize::new(0));
    let consumed: Arc<std::sync::Mutex<Vec<u64>>> = Arc::new(std::sync::Mutex::new(Vec::new()));

    scope(|s| {
        // Spawn consumers first so producers see drainage early.
        for _ in 0..CONSUMERS {
            let ring = ring.clone();
            let stop = Arc::clone(&stop_consumers);
            let sink = Arc::clone(&consumed);
            s.spawn(move |_| {
                let mut local: Vec<u64> = Vec::new();
                loop {
                    while let Some(item) = ring.pop() {
                        local.push(item);
                    }
                    // Exit once all producers signalled done AND queue
                    // really is empty across two probes (guards against
                    // a producer landing one last item between the
                    // two `pop` attempts).
                    if stop.load(Ordering::SeqCst) >= PRODUCERS {
                        if ring.pop().is_none() && ring.is_empty() {
                            break;
                        }
                    } else {
                        std::thread::yield_now();
                    }
                }
                sink.lock().unwrap().extend(local);
            });
        }

        for p in 0..PRODUCERS {
            let ring = ring.clone();
            let stop = Arc::clone(&stop_consumers);
            s.spawn(move |_| {
                // Encode `(producer_id, seq)` into a u64 so the
                // consumer can later reconstruct provenance.
                #[allow(clippy::cast_possible_truncation)]
                let base = (p as u64) << 32;
                for seq in 0..PER_PRODUCER {
                    ring.push(base | seq as u64);
                }
                stop.fetch_add(1, Ordering::SeqCst);
            });
        }
    })
    .unwrap();

    // Drain whatever's left now that all threads joined.
    while let Some(item) = ring.pop() {
        consumed.lock().unwrap().push(item);
    }

    let consumed = consumed.lock().unwrap().clone();
    let dropped = ring.dropped_count();

    // Invariants:
    //
    // 1. consumed + dropped == total pushed.
    let total_pushed = (PRODUCERS * PER_PRODUCER) as u64;
    let consumed_count = consumed.len() as u64;
    assert_eq!(
        consumed_count + dropped,
        total_pushed,
        "accounting mismatch: consumed={consumed_count} dropped={dropped} total_pushed={total_pushed}",
    );

    // 2. No corruption: every consumed item is a `(p, seq)` pair
    //    we actually pushed, and no item appears twice.
    let mut seen: HashSet<u64> = HashSet::with_capacity(consumed.len());
    for &item in &consumed {
        let p = (item >> 32) as usize;
        let seq = (item & 0xFFFF_FFFF) as usize;
        assert!(
            p < PRODUCERS,
            "consumed item from non-existent producer {p}"
        );
        assert!(
            seq < PER_PRODUCER,
            "consumed item with out-of-range seq {seq}"
        );
        assert!(seen.insert(item), "duplicate consumption of {item:#x}");
    }

    // 3. The queue must be empty at the end.
    assert!(ring.is_empty());
}

/// Single producer + single consumer, large run — confirms the
/// happy-path lossless mode under enough volume that scheduling slop
/// matters but the ring never overflows.
#[test]
fn lossless_when_capacity_exceeds_burst() {
    const CAPACITY: usize = 4096;
    const ITEMS: u64 = 100_000;

    let ring: Ring<u64> = Ring::with_capacity(CAPACITY).unwrap();
    let consumed: Arc<std::sync::Mutex<Vec<u64>>> = Arc::new(std::sync::Mutex::new(Vec::new()));

    scope(|s| {
        let drain_ring = ring.clone();
        let sink = Arc::clone(&consumed);
        let drain_handle = s.spawn(move |_| {
            let mut local: Vec<u64> = Vec::with_capacity(
                usize::try_from(ITEMS).expect("ITEMS fits in usize on the test host"),
            );
            let mut got: u64 = 0;
            while got < ITEMS {
                if let Some(item) = drain_ring.pop() {
                    local.push(item);
                    got += 1;
                } else {
                    std::thread::yield_now();
                }
            }
            sink.lock().unwrap().extend(local);
        });

        let feed_ring = ring.clone();
        s.spawn(move |_| {
            for i in 0..ITEMS {
                // Tight loop with a polite back-off to give the
                // drain task time to make room.
                while feed_ring.is_full() {
                    std::thread::yield_now();
                }
                feed_ring.push(i);
            }
        });

        drain_handle.join().unwrap();
    })
    .unwrap();

    assert_eq!(ring.dropped_count(), 0, "no overflow expected");
    let consumed = consumed.lock().unwrap();
    assert_eq!(consumed.len() as u64, ITEMS);
    // FIFO order preserved for a single-producer-single-consumer setup.
    for (i, &item) in consumed.iter().enumerate() {
        assert_eq!(item, i as u64, "out-of-order item at index {i}");
    }
}
