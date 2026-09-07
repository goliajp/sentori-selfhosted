//! # `sentori-rate-limiter` — backend-agnostic sliding-window rate limit
//!
//! Stone-tier crate (per cement-stone methodology) for the
//! rate-limit primitives Sentori composes against — today the
//! `push-provider` dispatcher and the self-hosted server's own
//! ingest / auth middleware.
//!
//! ## Architecture
//!
//! Four orthogonal pieces, each independently useful:
//!
//! - [`Policy`] — `(max_requests, window)` with construction-time
//!   validation. Reject 0 / overlong values up front so the hot
//!   path never has to defend against them.
//! - [`Clock`] trait — pluggable monotonic time. [`SystemClock`]
//!   wraps `Instant::now()`; [`MockClock`] is caller-driven and
//!   exposed (not feature-gated) so application code can write
//!   deterministic tests against `Limiter` without `thread::sleep`.
//! - [`RateBackend`] trait — storage abstraction. The stone ships
//!   [`MemoryBackend`] (sliding-window log; precise; O(max) per
//!   key), and that is the only backend that exists. A
//!   network-backed one would live in the 钢筋 layer above —
//!   stones can't take a network dep.
//! - [`Limiter`] — composes a backend + clock + policy into a
//!   one-call `check(key) -> Verdict` API.
//!
//! ## Why sliding-window log?
//!
//! Three common rate-limit algorithms exist:
//!
//! - **Fixed window** — count requests in the current N-second
//!   bucket. O(1) memory, but allows up to 2× the limit in any
//!   single window when a burst straddles a bucket boundary.
//! - **Sliding window counter** — count current bucket + weighted
//!   carryover from the previous. O(1) memory, ≤1.5× the limit
//!   in the worst case.
//! - **Sliding window log** — store the timestamp of every
//!   request inside the rolling window. O(max) memory, exact
//!   limit enforcement.
//!
//! The pre-v1 `server/src/rate_limit.rs` used fixed-window-minute
//! over Valkey; neither that tree nor Valkey survived the v1
//! redesign. The in-process backend goes with the log: at 30
//! req/min the per-key cost is 30 × 16 bytes = 480 bytes, and the
//! old 2× burst window cliff is gone. A cross-process backend
//! would trade precision for O(1) memory via the sliding-window-
//! counter technique; the same `Policy` + `Verdict` + `Limiter`
//! types would apply.
//!
//! ## Concurrency model
//!
//! [`MemoryBackend`] uses an internal `Mutex<HashMap<…>>` —
//! every `check` acquires the lock briefly. For 1000s of
//! concurrent callers against distinct keys this can become a
//! contention point, and single-process state does not survive a
//! second instance. Both would want a cross-process backend; none
//! is planned (see `docs/plans/kevy-migration.md`, parked). Both
//! [`MemoryBackend`] and [`Limiter`] are `Send + Sync`, meant to
//! be shared via `Arc`.
//!
//! ## Quick start
//!
//! ```rust
//! use core::time::Duration;
//! use sentori_rate_limiter::{Limiter, MemoryBackend, Policy, SystemClock};
//!
//! let policy = Policy::new(30, Duration::from_secs(60)).expect("policy");
//! let limiter = Limiter::new(MemoryBackend::new(), SystemClock, policy);
//!
//! // On every incoming request:
//! let verdict = limiter.check("token-hash-abc");
//! if verdict.is_limited() {
//!     // Surface as 429 Too Many Requests; the `retry_after`
//!     // duration goes in the Retry-After header.
//! }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::multiple_crate_versions)]

mod backend;
mod clock;
mod error;
mod limiter;
mod memory;
mod policy;

pub use backend::RateBackend;
pub use clock::{Clock, MockClock, SystemClock};
pub use error::PolicyError;
pub use limiter::Limiter;
pub use memory::MemoryBackend;
pub use policy::{Policy, Verdict};
