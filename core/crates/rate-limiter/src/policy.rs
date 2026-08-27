//! [`Policy`]: a validated `(max_requests, window)` pair, plus the
//! [`Verdict`] every `check` call returns.
//!
//! Inputs are validated at construction time so the limit-checking
//! hot path never has to defend against zero windows / zero limits.

use core::num::NonZeroU32;
use core::time::Duration;

use crate::error::PolicyError;

/// A rate-limit policy: at most `max_requests` requests per rolling
/// `window`.
///
/// Construct via [`Policy::new`]; values are validated up-front
/// so the runtime path can assume both fields are well-formed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Policy {
    max_requests: NonZeroU32,
    window: Duration,
}

impl Policy {
    /// Maximum sensible window — one hour. Longer windows would
    /// let `MemoryBackend`'s `VecDeque<Instant>` grow without
    /// bound on hot keys; pin a ceiling at construction time
    /// rather than discover it via OOM in prod.
    pub const MAX_WINDOW: Duration = Duration::from_hours(1);

    /// Build a new policy.
    ///
    /// # Errors
    ///
    /// - [`PolicyError::ZeroMaxRequests`] — `max` was 0.
    /// - [`PolicyError::ZeroWindow`] — `window` was zero.
    /// - [`PolicyError::WindowTooLong`] — `window` exceeded
    ///   [`Self::MAX_WINDOW`].
    pub fn new(max: u32, window: Duration) -> Result<Self, PolicyError> {
        let max = NonZeroU32::new(max).ok_or(PolicyError::ZeroMaxRequests)?;
        if window.is_zero() {
            return Err(PolicyError::ZeroWindow);
        }
        if window > Self::MAX_WINDOW {
            return Err(PolicyError::WindowTooLong);
        }
        Ok(Self {
            max_requests: max,
            window,
        })
    }

    /// Convenience constructor for the common "N per minute" shape.
    ///
    /// # Errors
    ///
    /// Same as [`Self::new`] minus `ZeroWindow` (60s is always
    /// non-zero) and `WindowTooLong` (60s is always ≤ 1h).
    pub fn per_minute(max: u32) -> Result<Self, PolicyError> {
        Self::new(max, Duration::from_mins(1))
    }

    /// Convenience for "N per second".
    ///
    /// # Errors
    ///
    /// Same as [`Self::per_minute`].
    pub fn per_second(max: u32) -> Result<Self, PolicyError> {
        Self::new(max, Duration::from_secs(1))
    }

    /// Convenience for "N per hour".
    ///
    /// # Errors
    ///
    /// Same as [`Self::per_minute`].
    pub fn per_hour(max: u32) -> Result<Self, PolicyError> {
        Self::new(max, Self::MAX_WINDOW)
    }

    /// Maximum number of requests permitted in any rolling
    /// `window` length of time.
    #[must_use]
    pub const fn max(self) -> u32 {
        self.max_requests.get()
    }

    /// The rolling-window duration.
    #[must_use]
    pub const fn window(self) -> Duration {
        self.window
    }
}

/// Result of a single rate-limit check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Request is within the limit.
    Allowed {
        /// How many more requests the caller can issue inside the
        /// current rolling window before they would hit the cap.
        /// `0` means this request consumed the last slot.
        remaining: u32,
    },
    /// Request was rejected because the limit is full.
    Limited {
        /// The minimum wait before retrying would succeed (the
        /// earliest of the existing log entries falls out of the
        /// window). Caller can surface as a `Retry-After` header.
        retry_after: Duration,
    },
}

impl Verdict {
    /// `true` iff the verdict permitted the request.
    #[must_use]
    pub const fn is_allowed(self) -> bool {
        matches!(self, Self::Allowed { .. })
    }

    /// `true` iff the verdict denied the request.
    #[must_use]
    pub const fn is_limited(self) -> bool {
        matches!(self, Self::Limited { .. })
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
    fn rejects_zero_max() {
        let err = Policy::new(0, Duration::from_mins(1)).expect_err("zero");
        assert!(matches!(err, PolicyError::ZeroMaxRequests));
    }

    #[test]
    fn rejects_zero_window() {
        let err = Policy::new(10, Duration::ZERO).expect_err("zero");
        assert!(matches!(err, PolicyError::ZeroWindow));
    }

    #[test]
    fn rejects_window_too_long() {
        let err = Policy::new(10, Duration::from_secs(3601)).expect_err("long");
        assert!(matches!(err, PolicyError::WindowTooLong));
    }

    #[test]
    fn accepts_window_at_ceiling() {
        let p = Policy::new(10, Policy::MAX_WINDOW).expect("at ceiling");
        assert_eq!(p.window(), Policy::MAX_WINDOW);
    }

    #[test]
    fn convenience_constructors() {
        assert_eq!(Policy::per_minute(10).unwrap().window().as_secs(), 60);
        assert_eq!(Policy::per_second(5).unwrap().window().as_secs(), 1);
        assert_eq!(Policy::per_hour(3).unwrap().window().as_secs(), 3600);
    }

    #[test]
    fn verdict_helpers() {
        let a = Verdict::Allowed { remaining: 3 };
        let l = Verdict::Limited {
            retry_after: Duration::from_millis(500),
        };
        assert!(a.is_allowed() && !a.is_limited());
        assert!(l.is_limited() && !l.is_allowed());
    }

    #[test]
    fn policy_round_trip() {
        let p = Policy::new(7, Duration::from_secs(90)).expect("ok");
        assert_eq!(p.max(), 7);
        assert_eq!(p.window(), Duration::from_secs(90));
    }
}
