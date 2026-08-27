//! Pluggable monotonic clock for the limiter.
//!
//! Two implementations:
//!
//! - [`SystemClock`] — wraps `std::time::Instant::now()`. Default
//!   for production use.
//! - [`MockClock`] — caller-driven, deterministic. Exposed in the
//!   public surface (not gated behind a feature) so application
//!   code can write deterministic tests against `Limiter` without
//!   resorting to `std::thread::sleep` (which is both slow and
//!   flaky in CI).

use core::time::Duration;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// A monotonic clock the limiter consults. `Send + Sync` so it
/// can be shared across worker threads via `Arc<C>` or by holding
/// the limiter behind an `Arc`.
pub trait Clock: Send + Sync {
    /// Current monotonic time.
    fn now(&self) -> Instant;
}

/// Blanket: `Arc<C>` is itself a `Clock` whenever `C` is. Lets
/// callers share one `MockClock` between the limiter and the
/// test driving it:
///
/// ```rust
/// use std::sync::Arc;
/// use std::time::{Duration, Instant};
/// use sentori_rate_limiter::{Clock, MockClock};
///
/// let clock = Arc::new(MockClock::new(Instant::now()));
/// // Hand `Arc::clone(&clock)` to the limiter; keep one yourself
/// // to call `clock.advance(...)` from the test body.
/// let now = (&*clock as &dyn Clock).now();
/// # let _ = now;
/// ```
impl<C: Clock + ?Sized> Clock for Arc<C> {
    fn now(&self) -> Instant {
        (**self).now()
    }
}

/// The real OS clock (`std::time::Instant::now()`). Use this in
/// production; pass [`MockClock`] in tests.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// A caller-driven clock. Start at an anchor `Instant` (typically
/// `Instant::now()` captured at the start of a test) and advance
/// it manually via [`MockClock::advance`].
///
/// Internal `Mutex` makes the clock `Send + Sync` so it can be
/// shared with the limiter across threads, the same way the
/// limiter's backend works.
#[derive(Debug)]
pub struct MockClock {
    current: Mutex<Instant>,
}

impl MockClock {
    /// Build a new mock clock starting at `anchor`.
    #[must_use]
    pub const fn new(anchor: Instant) -> Self {
        Self {
            current: Mutex::new(anchor),
        }
    }

    /// Advance the clock by `by`.
    pub fn advance(&self, by: Duration) {
        if let Ok(mut g) = self.current.lock() {
            *g += by;
        }
    }

    /// Reset the clock to `anchor`. Mostly useful for resetting
    /// state between test cases on a shared `MockClock`.
    pub fn reset(&self, anchor: Instant) {
        if let Ok(mut g) = self.current.lock() {
            *g = anchor;
        }
    }
}

impl Clock for MockClock {
    fn now(&self) -> Instant {
        // Poisoned mutex returns the anchor at construction time
        // — unusual, but better than panicking the limiter's hot
        // path on an unrelated panic in another caller.
        self.current.lock().map_or_else(|e| *e.into_inner(), |g| *g)
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
    fn system_clock_monotonic() {
        let c = SystemClock;
        let a = c.now();
        let b = c.now();
        assert!(b >= a);
    }

    #[test]
    fn mock_clock_starts_at_anchor() {
        let anchor = Instant::now();
        let c = MockClock::new(anchor);
        assert_eq!(c.now(), anchor);
    }

    #[test]
    fn mock_clock_advance() {
        let anchor = Instant::now();
        let c = MockClock::new(anchor);
        c.advance(Duration::from_secs(5));
        assert_eq!(c.now(), anchor + Duration::from_secs(5));
    }

    #[test]
    fn mock_clock_advance_is_cumulative() {
        let anchor = Instant::now();
        let c = MockClock::new(anchor);
        c.advance(Duration::from_secs(1));
        c.advance(Duration::from_secs(2));
        assert_eq!(c.now(), anchor + Duration::from_secs(3));
    }

    #[test]
    fn mock_clock_reset() {
        let anchor = Instant::now();
        let c = MockClock::new(anchor);
        c.advance(Duration::from_secs(10));
        let new_anchor = anchor + Duration::from_secs(100);
        c.reset(new_anchor);
        assert_eq!(c.now(), new_anchor);
    }
}
