//! Error surface for [`crate::Ring`].

use thiserror::Error;

/// Convenience [`Result`] alias.
pub type RingResult<T> = Result<T, CapacityError>;

/// Rejected ring-buffer construction parameters.
///
/// The single error variant covers the only configuration-time
/// rejection the crate makes — choosing a capacity of zero, which
/// `crossbeam_queue::ArrayQueue` itself would panic on.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CapacityError {
    /// Requested capacity was zero.
    ///
    /// A zero-capacity ring would accept no events and report 100 %
    /// drops — never useful and indistinguishable in practice from
    /// "ring buffer not wired up". We reject up front so the misuse
    /// surfaces immediately at construction.
    #[error("ring capacity must be >= 1, got 0")]
    Zero,
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
    fn zero_message_clear() {
        assert_eq!(
            CapacityError::Zero.to_string(),
            "ring capacity must be >= 1, got 0",
        );
    }

    #[test]
    fn debug_impl() {
        let _ = format!("{:?}", CapacityError::Zero);
    }

    #[test]
    fn equality_holds() {
        assert_eq!(CapacityError::Zero, CapacityError::Zero);
    }
}
