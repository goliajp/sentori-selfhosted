//! Constant-time equality for shared secrets that are not one of the
//! typed primitives above.
//!
//! `SignedCookie::verify`, `EncryptedCookie::open` and `CsrfToken::verify`
//! already compare in constant time, because that is the crate's job.
//! Callers with a raw secret that never turned into one of those
//! wrappers — an OAuth state cookie, a `Bearer` shared between a
//! browser and the server — used to reach for `==` and quietly
//! reintroduce a timing leak the rest of the crate takes care to
//! prevent.
//!
//! This is the same primitive `subtle::ConstantTimeEq` gives, in a
//! signature that requires no new dependency at the call site and no
//! remembering of which `Choice` conversion to use.

use subtle::ConstantTimeEq;

/// Return `true` iff the two byte slices are equal, in time that does
/// not depend on where they differ.
///
/// Length mismatch returns `false` early. That is not a secret worth
/// hiding — the sizes of the secrets this crate compares are all
/// fixed and known (32-byte HMAC tags, 32-byte cookie values, 64-char
/// hex OAuth state) — and returning `false` early avoids padding the
/// shorter side, which would itself be a data-dependent branch.
#[must_use]
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.ct_eq(b).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_bytes_return_true() {
        assert!(constant_time_eq(b"hello", b"hello"));
        assert!(constant_time_eq(b"", b""));
        assert!(constant_time_eq(&[0u8; 32], &[0u8; 32]));
    }

    #[test]
    fn different_bytes_return_false() {
        assert!(!constant_time_eq(b"hello", b"world"));
        assert!(!constant_time_eq(b"hell", b"help"));
        // Even a single-bit difference deep in the buffer must fail.
        let mut a = [0u8; 32];
        let mut b = [0u8; 32];
        b[31] = 1;
        assert!(!constant_time_eq(&a, &b));
        a[0] = 1;
        b[0] = 1;
        assert!(!constant_time_eq(&a, &b));
    }

    #[test]
    fn length_mismatch_returns_false() {
        assert!(!constant_time_eq(b"a", b"aa"));
        assert!(!constant_time_eq(b"", b"a"));
    }
}
