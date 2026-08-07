//! Single-use email-delivered token: random 32B base64url on
//! the wire, SHA-256 hash in DB. Same shape as K1
//! [`sentori_workspace_identity::InviteToken`].
//!
//! Generic over a phantom marker so [`EmailVerifyToken`] and
//! [`PasswordResetToken`] are typed-distinct at the API surface
//! while sharing the bytes-handling code. Mixing one up with
//! the other (passing an email-verify token to
//! `reset_password`) is a compile error.

use std::fmt;
use std::marker::PhantomData;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use zeroize::Zeroize;

use crate::error::AuthError;

/// 32 bytes of random == 256 bits of entropy. 43 base64url-no-
/// pad chars on the wire.
pub const TOKEN_BYTES: usize = 32;

/// SHA-256 hash of a single-use token's plaintext bytes.
///
/// 32 bytes. The hash is what we store in the DB column; the
/// plaintext leaves the server exactly once (in the `Minted*`
/// return values).
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct SingleUseTokenHash(pub [u8; 32]);

impl SingleUseTokenHash {
    /// Borrow as a `BYTEA` parameter slice.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Constant-time equality check.
    #[must_use]
    pub fn ct_eq(&self, other: &Self) -> bool {
        self.0.ct_eq(&other.0).into()
    }
}

impl fmt::Debug for SingleUseTokenHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SingleUseTokenHash")
            .field(&hex(&self.0))
            .finish()
    }
}

/// Plaintext single-use token. Carries 32 random bytes; Zeroes
/// on Drop. Generic over a phantom marker so typed wrappers
/// stay distinct at the API surface.
pub struct SingleUseToken<M> {
    bytes: [u8; TOKEN_BYTES],
    _marker: PhantomData<M>,
}

impl<M> SingleUseToken<M> {
    /// Mint a fresh random token from the OS CSPRNG.
    ///
    /// # Errors
    ///
    /// [`AuthError::Entropy`] if the OS RNG is unavailable.
    pub fn generate() -> Result<Self, AuthError> {
        let mut bytes = [0u8; TOKEN_BYTES];
        getrandom::getrandom(&mut bytes).map_err(|e| AuthError::Entropy(e.to_string()))?;
        Ok(Self {
            bytes,
            _marker: PhantomData,
        })
    }

    /// Encode in 43-char base64url-no-pad wire form.
    #[must_use]
    pub fn to_wire_string(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.bytes)
    }

    /// SHA-256 of the token bytes — what gets stored in DB.
    #[must_use]
    pub fn hash(&self) -> SingleUseTokenHash {
        SingleUseTokenHash(sha256(&self.bytes))
    }

    /// Parse a wire-format string into the DB-side hash. Cleartext
    /// stays on the stack and is zeroed on return.
    ///
    /// # Errors
    ///
    /// [`AuthError::TokenInvalid`] for malformed input.
    pub fn parse_and_hash(s: &str) -> Result<SingleUseTokenHash, AuthError> {
        let mut decoded = URL_SAFE_NO_PAD
            .decode(s.as_bytes())
            .map_err(|_| AuthError::TokenInvalid)?;
        if decoded.len() != TOKEN_BYTES {
            decoded.zeroize();
            return Err(AuthError::TokenInvalid);
        }
        let mut buf = [0u8; TOKEN_BYTES];
        buf.copy_from_slice(&decoded);
        decoded.zeroize();
        let hash = sha256(&buf);
        buf.zeroize();
        Ok(SingleUseTokenHash(hash))
    }
}

impl<M> Drop for SingleUseToken<M> {
    fn drop(&mut self) {
        self.bytes.zeroize();
    }
}

impl<M> fmt::Debug for SingleUseToken<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SingleUseToken")
            .field("bytes", &"<redacted>")
            .finish()
    }
}

/// Marker for [`EmailVerifyToken`].
#[derive(Debug, Clone, Copy)]
pub struct EmailVerifyMarker;

/// Marker for [`PasswordResetToken`].
#[derive(Debug, Clone, Copy)]
pub struct PasswordResetMarker;

/// Single-use email-verification token. Wraps [`SingleUseToken`]
/// with a typed marker so it can't be confused with a password-
/// reset token at the API surface.
pub type EmailVerifyToken = SingleUseToken<EmailVerifyMarker>;

/// Single-use password-reset token. Wraps [`SingleUseToken`]
/// with a typed marker so it can't be confused with an email-
/// verify token at the API surface.
pub type PasswordResetToken = SingleUseToken<PasswordResetMarker>;

fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write as _;
        let _ = write!(s, "{b:02x}");
    }
    s
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_email_verify() {
        let t = EmailVerifyToken::generate().expect("gen");
        let wire = t.to_wire_string();
        assert_eq!(wire.len(), 43);
        let parsed = EmailVerifyToken::parse_and_hash(&wire).expect("parse");
        assert!(t.hash().ct_eq(&parsed));
    }

    #[test]
    fn round_trip_password_reset() {
        let t = PasswordResetToken::generate().expect("gen");
        let wire = t.to_wire_string();
        let parsed = PasswordResetToken::parse_and_hash(&wire).expect("parse");
        assert!(t.hash().ct_eq(&parsed));
    }

    #[test]
    fn rejects_wrong_length() {
        let short = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode([1u8; 16]);
        assert!(matches!(
            EmailVerifyToken::parse_and_hash(&short),
            Err(AuthError::TokenInvalid)
        ));
    }

    #[test]
    fn debug_does_not_leak_bytes() {
        let t = EmailVerifyToken::generate().expect("gen");
        let s = format!("{t:?}");
        assert!(s.contains("redacted"));
        assert!(!s.contains(&t.to_wire_string()));
    }
}
