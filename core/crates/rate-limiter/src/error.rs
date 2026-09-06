//! Error types for the rate-limiter stone.

use core::fmt;

/// Errors returned by [`crate::Policy::new`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PolicyError {
    /// `max_requests` was 0 — a "no requests allowed" limit makes
    /// the limiter degenerate. Use a `Result<…, ZeroVerdict>` at
    /// the application layer if that's the desired shape.
    ZeroMaxRequests,
    /// `window` was zero duration — a 0-second window makes
    /// "current bucket" undefined. Use a feature flag at the
    /// application layer if you want to bypass the limiter
    /// entirely.
    ZeroWindow,
    /// `window` exceeded [`crate::Policy::MAX_WINDOW`] (1 hour).
    /// Longer windows would let the in-memory backend's per-key
    /// `VecDeque<Instant>` grow without bound on hot keys.
    WindowTooLong,
}

impl fmt::Display for PolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroMaxRequests => f.write_str("Policy max_requests must be > 0"),
            Self::ZeroWindow => f.write_str("Policy window must be > 0s"),
            Self::WindowTooLong => f.write_str("Policy window exceeds Policy::MAX_WINDOW (1h)"),
        }
    }
}

impl std::error::Error for PolicyError {}
