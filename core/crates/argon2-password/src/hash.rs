//! Hash / verify entry points.

use argon2::{Algorithm, Argon2, Version};
use password_hash::rand_core::OsRng;
use password_hash::{PasswordHash as PhcHash, PasswordHasher, PasswordVerifier, SaltString};
use zeroize::Zeroize;

use crate::error::{PasswordError, PasswordResult};
use crate::params::Params;

/// Maximum password length this stone accepts, in bytes.
///
/// 1 KiB. Not an algorithmic limit — Argon2 itself happily
/// hashes any length — but a defence against a malicious caller
/// asking us to hash multi-megabyte strings and tying up an
/// Argon2 worker for seconds. Real password fields cap at 64-
/// 128 chars in practice; 1 KiB leaves room for passphrases.
pub const MAX_PASSWORD_BYTES: usize = 1024;

/// Stateless namespace for Argon2id hashing.
///
/// No fields; every method is associated. Mirrors S9's
/// `cookie-session::PasswordHash` shape exactly so the
/// migration path between the two stones is "swap the import".
pub struct PasswordHash;

impl PasswordHash {
    /// Hash `password` with [`Params::OWASP_2025`].
    ///
    /// Equivalent to
    /// `Self::hash_with_params(password, Params::OWASP_2025)`.
    ///
    /// # Errors
    ///
    /// See [`Self::hash_with_params`].
    pub fn hash(password: &str) -> PasswordResult<String> {
        Self::hash_with_params(password, Params::OWASP_2025)
    }

    /// Hash `password` with the given [`Params`].
    ///
    /// Returns a PHC-format string ready to drop into a
    /// `users.password_hash` column.
    ///
    /// # Errors
    ///
    /// - [`PasswordError::TooLong`] — password exceeds
    ///   [`MAX_PASSWORD_BYTES`].
    /// - [`PasswordError::InvalidParams`] — `params` failed
    ///   [`Params::validate`].
    /// - [`PasswordError::EntropyFailure`] — OS CSPRNG failed
    ///   to generate the salt.
    /// - [`PasswordError::MalformedHash`] — argon2 internal
    ///   failure (extremely rare; corrupt state).
    pub fn hash_with_params(password: &str, params: Params) -> PasswordResult<String> {
        if password.len() > MAX_PASSWORD_BYTES {
            return Err(PasswordError::TooLong);
        }
        params.validate()?;

        let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params.to_argon2()?);
        let salt = SaltString::generate(&mut OsRng);
        let hash = argon
            .hash_password(password.as_bytes(), &salt)
            .map_err(map_phc_err)?
            .to_string();
        Ok(hash)
    }

    /// Verify `password` against a previously-stored PHC-format
    /// hash.
    ///
    /// Returns `Ok(true)` on match, `Ok(false)` on mismatch.
    /// The parse / verify path is constant-time relative to the
    /// stored params (the params themselves are publicly visible
    /// in the PHC string, so the timing leakage they cause is
    /// intentional).
    ///
    /// # Errors
    ///
    /// - [`PasswordError::TooLong`] — password exceeds
    ///   [`MAX_PASSWORD_BYTES`].
    /// - [`PasswordError::MalformedHash`] — `stored` is not a
    ///   valid argon2 PHC string.
    pub fn verify(password: &str, stored: &str) -> PasswordResult<bool> {
        if password.len() > MAX_PASSWORD_BYTES {
            return Err(PasswordError::TooLong);
        }
        let parsed = PhcHash::new(stored).map_err(|_| PasswordError::MalformedHash)?;
        // Reject non-argon2 PHC strings (e.g. bcrypt `$2b$…`)
        // up front so the caller's "wrong algorithm" branch is
        // a typed error rather than `Ok(false)`.
        if parsed.algorithm.as_str() != "argon2id"
            && parsed.algorithm.as_str() != "argon2i"
            && parsed.algorithm.as_str() != "argon2d"
        {
            return Err(PasswordError::MalformedHash);
        }
        let argon = Argon2::default();
        let result = argon.verify_password(password.as_bytes(), &parsed);
        match result {
            Ok(()) => Ok(true),
            Err(password_hash::Error::Password) => Ok(false),
            Err(_) => Err(PasswordError::MalformedHash),
        }
    }

    /// Hash `password`, taking ownership of a `String` so the
    /// caller's plaintext can be zeroed on consumption.
    ///
    /// Convenience for forms where the plaintext password is
    /// received as `String` (e.g. axum `Json<RegisterReq>`).
    /// Zeroes the input after hashing, so the caller doesn't
    /// need to remember `password.zeroize()`.
    ///
    /// # Errors
    ///
    /// Same as [`Self::hash_with_params`].
    pub fn hash_owned(mut password: String) -> PasswordResult<String> {
        let result = Self::hash_with_params(&password, Params::OWASP_2025);
        password.zeroize();
        result
    }

    /// Verify-then-zero variant of [`Self::verify`].
    ///
    /// # Errors
    ///
    /// Same as [`Self::verify`].
    pub fn verify_owned(mut password: String, stored: &str) -> PasswordResult<bool> {
        let result = Self::verify(&password, stored);
        password.zeroize();
        result
    }
}

const fn map_phc_err(err: password_hash::Error) -> PasswordError {
    match err {
        password_hash::Error::OutputSize { .. } => PasswordError::InvalidParams("output size"),
        password_hash::Error::ParamValueInvalid(_)
        | password_hash::Error::ParamNameInvalid
        | password_hash::Error::ParamsMaxExceeded => {
            PasswordError::InvalidParams("argon2 internal param rejection")
        }
        _ => PasswordError::MalformedHash,
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

    // Argon2 is slow by design; use Params::TEST_FAST in tests
    // (8 KiB / 1 iter) so the unit suite stays in the ms range.
    fn fast() -> Params {
        Params::TEST_FAST
    }

    #[test]
    fn hash_then_verify_round_trip() {
        let hash = PasswordHash::hash_with_params("hunter2", fast()).expect("hash");
        assert!(PasswordHash::verify("hunter2", &hash).expect("verify"));
    }

    #[test]
    fn verify_rejects_wrong_password() {
        let hash = PasswordHash::hash_with_params("hunter2", fast()).expect("hash");
        assert!(!PasswordHash::verify("hunter3", &hash).expect("verify"));
    }

    #[test]
    fn hash_is_unique_per_call() {
        // Random per-call salt — same input + params must
        // produce distinct hashes.
        let a = PasswordHash::hash_with_params("hunter2", fast()).expect("hash");
        let b = PasswordHash::hash_with_params("hunter2", fast()).expect("hash");
        assert_ne!(a, b);
        assert!(PasswordHash::verify("hunter2", &a).expect("verify"));
        assert!(PasswordHash::verify("hunter2", &b).expect("verify"));
    }

    #[test]
    fn hash_format_is_phc_argon2id() {
        let hash = PasswordHash::hash_with_params("hunter2", fast()).expect("hash");
        // PHC argon2id: $argon2id$v=19$m=...,t=...,p=...$<salt>$<hash>
        assert!(hash.starts_with("$argon2id$v=19$"));
    }

    #[test]
    fn rejects_password_over_max_bytes() {
        let too_long = "x".repeat(MAX_PASSWORD_BYTES + 1);
        let err = PasswordHash::hash_with_params(&too_long, fast()).expect_err("reject");
        assert!(matches!(err, PasswordError::TooLong));
    }

    #[test]
    fn rejects_verify_with_password_over_max_bytes() {
        let valid = PasswordHash::hash_with_params("ok", fast()).expect("hash");
        let too_long = "x".repeat(MAX_PASSWORD_BYTES + 1);
        let err = PasswordHash::verify(&too_long, &valid).expect_err("too long");
        assert!(matches!(err, PasswordError::TooLong));
    }

    #[test]
    fn accepts_password_at_boundary() {
        let exactly = "x".repeat(MAX_PASSWORD_BYTES);
        let hash = PasswordHash::hash_with_params(&exactly, fast()).expect("hash at boundary");
        assert!(PasswordHash::verify(&exactly, &hash).expect("verify"));
    }

    #[test]
    fn rejects_malformed_stored_hash() {
        let err = PasswordHash::verify("hunter2", "not a phc hash").expect_err("malformed");
        assert!(matches!(err, PasswordError::MalformedHash));
    }

    #[test]
    fn rejects_bcrypt_hash() {
        // A real bcrypt hash should NOT verify against this stone.
        let bcrypt_like = "$2b$05$abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJKLMNOPQRSTUVW";
        let err = PasswordHash::verify("hunter2", bcrypt_like).expect_err("bcrypt rejected");
        assert!(matches!(err, PasswordError::MalformedHash));
    }

    #[test]
    fn rejects_empty_stored_hash() {
        let err = PasswordHash::verify("hunter2", "").expect_err("empty");
        assert!(matches!(err, PasswordError::MalformedHash));
    }

    #[test]
    fn empty_password_round_trips() {
        let hash = PasswordHash::hash_with_params("", fast()).expect("hash");
        assert!(PasswordHash::verify("", &hash).expect("verify"));
        assert!(!PasswordHash::verify("nonempty", &hash).expect("verify"));
    }

    #[test]
    fn rejects_params_t_cost_zero() {
        let p = Params {
            m_cost: 8,
            t_cost: 0,
            p_cost: 1,
        };
        let err = PasswordHash::hash_with_params("hunter2", p).expect_err("invalid params");
        assert!(matches!(err, PasswordError::InvalidParams(_)));
    }

    #[test]
    fn rejects_params_m_cost_too_small() {
        let p = Params {
            m_cost: 0,
            t_cost: 1,
            p_cost: 1,
        };
        let err = PasswordHash::hash_with_params("hunter2", p).expect_err("invalid params");
        assert!(matches!(err, PasswordError::InvalidParams(_)));
    }

    #[test]
    fn rejects_params_p_cost_too_large() {
        let p = Params {
            m_cost: 8,
            t_cost: 1,
            p_cost: 17,
        };
        let err = PasswordHash::hash_with_params("hunter2", p).expect_err("invalid params");
        assert!(matches!(err, PasswordError::InvalidParams(_)));
    }

    #[test]
    fn default_params_match_owasp_2025() {
        assert_eq!(Params::default(), Params::OWASP_2025);
    }

    #[test]
    fn hash_owned_zeroes_input_and_verifies() {
        let input = String::from("hunter2");
        let hash = PasswordHash::hash_owned(input).expect("hash");
        assert!(PasswordHash::verify("hunter2", &hash).expect("verify"));
    }

    #[test]
    fn verify_owned_zeroes_input() {
        let hash = PasswordHash::hash_with_params("hunter2", fast()).expect("hash");
        let input = String::from("hunter2");
        assert!(PasswordHash::verify_owned(input, &hash).expect("verify"));
    }

    #[test]
    fn higher_params_still_verify() {
        // OWASP-2025 hash should still verify even though it's
        // ~3000x slower than TEST_FAST. Single-call test.
        let hash = PasswordHash::hash_with_params("slow", Params::OWASP_2025).expect("hash");
        assert!(PasswordHash::verify("slow", &hash).expect("verify"));
    }
}
