//! Bcrypt password hashing.
//!
//! Wraps the `bcrypt` crate (RustCrypto-style pure-Rust impl of
//! OpenBSD's bcrypt(5) algorithm) with a typed surface:
//!
//! - [`PasswordHash::hash`] — bcrypt a password at a configurable
//!   cost, return the modular-crypt-format string (`$2b$cost$salt+hash`)
//!   suitable for direct storage in a `users.password_hash` column.
//! - [`PasswordHash::verify`] — verify a candidate password against
//!   a stored hash; constant-time on success/failure.
//!
//! ## Design notes
//!
//! - **No silent truncation.** bcrypt's algorithm only consumes
//!   the first 72 bytes of input. Many implementations silently
//!   truncate longer passwords, which means
//!   `"<70-byte pass>foobar"` and `"<70-byte pass>xxxxxx"` produce
//!   identical hashes — a real attack vector. We refuse and
//!   return [`crate::PasswordError::TooLong`]; the caller must
//!   explicitly hash-then-bcrypt (typically SHA-256-first) if they
//!   want to accept longer passwords.
//! - **Cost knob.** The default of [`PasswordHash::COST_DEFAULT`]
//!   (12) is what every contemporary Rust web stack picks for
//!   v0.x. Bump it as hardware speeds up; the stored format
//!   carries the cost so old hashes still verify after a bump.

use crate::error::{PasswordError, PasswordResult};

/// Stateless namespace for the password-hash primitive.
pub struct PasswordHash;

/// Maximum input length bcrypt's algorithm consumes — bytes past
/// this would be silently dropped, so we refuse them up front.
const MAX_PASSWORD_BYTES: usize = 72;

impl PasswordHash {
    /// Default bcrypt cost factor. `2^12 ≈ 4096` rounds; ~300 ms
    /// on a 2023 laptop core, balanced against UX. Bump as
    /// hardware speeds up.
    pub const COST_DEFAULT: u32 = 12;

    /// The minimum cost bcrypt accepts. Provided for callers that
    /// want to validate user-supplied cost choices before calling.
    pub const COST_MIN: u32 = bcrypt::DEFAULT_COST.saturating_sub(8); // 4 in practice

    /// The maximum cost bcrypt accepts.
    pub const COST_MAX: u32 = 31;

    /// Hash `password` at [`Self::COST_DEFAULT`].
    ///
    /// Equivalent to `Self::hash_with_cost(password, COST_DEFAULT)`.
    ///
    /// # Errors
    ///
    /// See [`Self::hash_with_cost`].
    pub fn hash(password: &str) -> PasswordResult<String> {
        Self::hash_with_cost(password, Self::COST_DEFAULT)
    }

    /// Hash `password` at the given bcrypt cost.
    ///
    /// # Errors
    ///
    /// - [`PasswordError::TooLong`] — password exceeds 72 bytes
    ///   (bcrypt's input ceiling).
    /// - [`PasswordError::InvalidCost`] — `cost` is outside the
    ///   `4..=31` range bcrypt accepts.
    /// - [`PasswordError::EntropyFailure`] — OS CSPRNG salt
    ///   generation failed.
    pub fn hash_with_cost(password: &str, cost: u32) -> PasswordResult<String> {
        if password.len() > MAX_PASSWORD_BYTES {
            return Err(PasswordError::TooLong);
        }
        if !(Self::COST_MIN..=Self::COST_MAX).contains(&cost) {
            return Err(PasswordError::InvalidCost);
        }
        match bcrypt::hash(password, cost) {
            Ok(s) => Ok(s),
            Err(bcrypt::BcryptError::Rand(_)) => Err(PasswordError::EntropyFailure),
            Err(bcrypt::BcryptError::CostNotAllowed(_)) => Err(PasswordError::InvalidCost),
            Err(_) => Err(PasswordError::MalformedHash),
        }
    }

    /// Verify `password` against a previously-stored bcrypt hash.
    ///
    /// Returns `Ok(true)` if the password matches, `Ok(false)` if
    /// it doesn't. Either branch runs in constant-time relative to
    /// the bcrypt cost (the timing leaks only the cost factor,
    /// which is intentionally public).
    ///
    /// # Errors
    ///
    /// - [`PasswordError::MalformedHash`] — `stored_hash` is not a
    ///   valid bcrypt(5) modular crypt format string.
    /// - [`PasswordError::TooLong`] — `password` exceeds 72 bytes.
    pub fn verify(password: &str, stored_hash: &str) -> PasswordResult<bool> {
        if password.len() > MAX_PASSWORD_BYTES {
            return Err(PasswordError::TooLong);
        }
        bcrypt::verify(password, stored_hash).map_err(|e| match e {
            bcrypt::BcryptError::Rand(_) => PasswordError::EntropyFailure,
            // Every other variant (InvalidHash, CostNotAllowed,
            // Truncation, Io, …) signals "the stored hash isn't
            // a bcrypt(5) modular crypt format we recognise".
            _ => PasswordError::MalformedHash,
        })
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

    // bcrypt is slow by design — we use the minimum cost in tests
    // so the suite doesn't take 30s. Hash format is identical
    // across costs, so verification semantics are preserved.
    const TEST_COST: u32 = PasswordHash::COST_MIN;

    #[test]
    fn hash_then_verify_round_trip() {
        let hash = PasswordHash::hash_with_cost("hunter2", TEST_COST).expect("hash");
        assert!(PasswordHash::verify("hunter2", &hash).expect("verify"));
    }

    #[test]
    fn verify_rejects_wrong_password() {
        let hash = PasswordHash::hash_with_cost("hunter2", TEST_COST).expect("hash");
        assert!(!PasswordHash::verify("hunter3", &hash).expect("verify"));
    }

    #[test]
    fn hash_is_unique_per_call() {
        // bcrypt uses a per-call random salt — same input + cost
        // must produce distinct outputs.
        let a = PasswordHash::hash_with_cost("hunter2", TEST_COST).expect("hash");
        let b = PasswordHash::hash_with_cost("hunter2", TEST_COST).expect("hash");
        assert_ne!(a, b);
        // Both must still verify the same password.
        assert!(PasswordHash::verify("hunter2", &a).expect("verify"));
        assert!(PasswordHash::verify("hunter2", &b).expect("verify"));
    }

    #[test]
    fn hash_format_is_modular_crypt() {
        let hash = PasswordHash::hash_with_cost("hunter2", TEST_COST).expect("hash");
        // bcrypt(5) format: $2[abxy]$cost$22-char-salt + 31-char-hash
        assert!(hash.starts_with("$2"));
        assert_eq!(hash.len(), 60);
    }

    #[test]
    fn rejects_password_over_72_bytes() {
        let too_long = "x".repeat(73);
        let err = PasswordHash::hash_with_cost(&too_long, TEST_COST).expect_err("must reject");
        assert!(matches!(err, PasswordError::TooLong));
    }

    #[test]
    fn rejects_verify_with_password_over_72_bytes() {
        let valid = PasswordHash::hash_with_cost("ok", TEST_COST).expect("hash");
        let too_long = "x".repeat(73);
        let err = PasswordHash::verify(&too_long, &valid).expect_err("too long");
        assert!(matches!(err, PasswordError::TooLong));
    }

    #[test]
    fn accepts_password_at_72_byte_boundary() {
        let exactly_72 = "x".repeat(MAX_PASSWORD_BYTES);
        let hash = PasswordHash::hash_with_cost(&exactly_72, TEST_COST).expect("at boundary");
        assert!(PasswordHash::verify(&exactly_72, &hash).expect("verify"));
    }

    #[test]
    fn rejects_cost_below_minimum() {
        let err = PasswordHash::hash_with_cost("hunter2", 3).expect_err("too low");
        assert!(matches!(err, PasswordError::InvalidCost));
    }

    #[test]
    fn rejects_cost_above_maximum() {
        let err = PasswordHash::hash_with_cost("hunter2", 32).expect_err("too high");
        assert!(matches!(err, PasswordError::InvalidCost));
    }

    #[test]
    fn rejects_malformed_stored_hash() {
        let err = PasswordHash::verify("hunter2", "not a bcrypt hash").expect_err("malformed");
        assert!(matches!(err, PasswordError::MalformedHash));
    }

    #[test]
    fn rejects_empty_stored_hash() {
        let err = PasswordHash::verify("hunter2", "").expect_err("empty");
        assert!(matches!(err, PasswordError::MalformedHash));
    }

    #[test]
    fn empty_password_round_trips() {
        let hash = PasswordHash::hash_with_cost("", TEST_COST).expect("hash");
        assert!(PasswordHash::verify("", &hash).expect("verify"));
        assert!(!PasswordHash::verify("nonempty", &hash).expect("verify"));
    }

    #[test]
    fn cost_min_is_4() {
        assert_eq!(PasswordHash::COST_MIN, 4);
    }

    #[test]
    fn cost_max_is_31() {
        assert_eq!(PasswordHash::COST_MAX, 31);
    }
}
