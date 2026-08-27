//! # `sentori-event-ringbuffer` — bounded lock-free ingest buffer
//!
//! Stone-tier crate (per cement-stone methodology) supplying the
//! hand-off queue Sentori's ingest pipeline uses between the HTTP
//! handlers (many concurrent producers) and the persistence drainer
//! (single consumer). Built on top of [`crossbeam_queue::ArrayQueue`]
//! so the lock-free atomics live in a well-audited upstream and our
//! crate stays under `#![forbid(unsafe_code)]`.
//!
//! ## What it adds over `ArrayQueue`
//!
//! Two things the upstream queue doesn't provide:
//!
//! 1. **Drop-oldest overflow.** When the queue is full, a [`Ring`]
//!    evicts the oldest queued item and pushes the new one. Sentori's
//!    ingest is a "live tail" — newer events carry more
//!    actionable signal than the oldest waiting one (a 10-minute-old
//!    queued report is far less useful than the crash that just
//!    happened). Under heavy contention the eviction is best-effort:
//!    a concurrent producer can fill the freshly freed slot, in
//!    which case the new item is dropped instead. Either path bumps
//!    the drop counter (see below).
//!
//! 2. **Atomic drop telemetry.** Every dropped item — whether by
//!    eviction or by the lose-the-race fallback — is counted in an
//!    [`AtomicU64`](std::sync::atomic::AtomicU64). The counter is
//!    accessible without locking the producer path, so observability
//!    code can plot drops over time without backpressure on the
//!    ingest hot path. This is load-bearing for the project's
//!    "Sentori must not perturb the host app" guarantee
//!    (`.claude/CLAUDE.md` section on performance).
//!
//! ## Semantics in one paragraph
//!
//! [`Ring::push`] returns a [`PushOutcome`] telling the caller whether
//! the item landed cleanly ([`PushOutcome::Inserted`]), was inserted
//! after evicting the oldest queued item
//! ([`PushOutcome::InsertedAfterEviction`]), or was itself dropped
//! because the queue stayed full ([`PushOutcome::Dropped`]). The drop
//! counter reflects only the two latter cases — it counts events
//! lost, not events that perturbed the queue.
//!
//! ## Quick start
//!
//! ```rust
//! use sentori_event_ringbuffer::{PushOutcome, Ring};
//!
//! # fn main() -> Result<(), sentori_event_ringbuffer::CapacityError> {
//! let ring: Ring<u32> = Ring::with_capacity(4)?;
//!
//! // Producer side — HTTP handlers in production, just one thread here.
//! for i in 0..4 {
//!     assert_eq!(ring.push(i), PushOutcome::Inserted);
//! }
//! assert_eq!(ring.len(), 4);
//!
//! // The queue is full — pushing `4` evicts the oldest (`0`).
//! assert_eq!(ring.push(4), PushOutcome::InsertedAfterEviction);
//! assert_eq!(ring.dropped_count(), 1);
//!
//! // Consumer side — pops FIFO.
//! assert_eq!(ring.pop(), Some(1));
//! assert_eq!(ring.pop(), Some(2));
//! assert_eq!(ring.pop(), Some(3));
//! assert_eq!(ring.pop(), Some(4));
//! assert_eq!(ring.pop(), None);
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]
// Crate docs mix English prose with technical pseudocode the
// doc_markdown heuristic mis-flags ("MPMC", "HTTP", "FIFO", etc.).
#![allow(clippy::doc_markdown)]

mod error;
mod ring;

pub use error::{CapacityError, RingResult};
pub use ring::{MIN_CAPACITY, PushOutcome, Ring};
