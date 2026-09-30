//! CSRF token generation + constant-time verification.
//!
//! [`CsrfToken`] wraps a 32-byte random secret encoded as
//! base64-url-no-pad. Generate one per logged-in session, hand it
//! to the client via a Set-Cookie or rendered into the page, and
//! require the client to echo it back on every state-changing
//! request (POST / PUT / DELETE). The server then verifies the
//! echoed token equals the one bound to the session.
//!
//! ## What this primitive deliberately does not do
//!
//! - **No session binding.** The token is content; how the caller
//!   ties it to a specific user / session lives in the 钢筋
//!   layer (typically: store the token in the session cookie,
//!   require the client to echo it in a `X-CSRF-Token` header).
//! - **No expiration.** Tokens are valid until the surrounding
//!   session is destroyed; rotating mid-session is a caller-level
//!   choice.
//! - **No double-submit / cookie-name policy.** Both are 钢筋-layer
//!   concerns — the stone keeps to the primitive.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use subtle::ConstantTimeEq;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::error::{CsrfError, CsrfResult};

/// Length (bytes) of the raw token. 32 bytes = 256 bits = far
/// more entropy than an attacker can hope to brute-force.
pub const TOKEN_LEN: usize = 32;

/// A CSRF token: 32 bytes of OS-entropy, encoded as
/// base64-url-no-pad on the wire.
///
/// Zero-on-drop so a token forgotten in a stack frame doesn't
/// stick around in process memory. Equality is constant-time.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct CsrfToken([u8; TOKEN_LEN]);

impl CsrfToken {
    /// Generate a fresh CSRF token via the OS CSPRNG.
    ///
    /// # Errors
    ///
    /// - [`CsrfError::EntropyFailure`] — `getrandom` returned an
    ///   error. Practically never happens on a healthy system.
    pub fn generate() -> CsrfResult<Self> {
        let mut buf = [0u8; TOKEN_LEN];
        getrandom::getrandom(&mut buf).map_err(|_| CsrfError::EntropyFailure)?;
        Ok(Self(buf))
    }

    /// Encode the token for the wire (Set-Cookie value, hidden
    /// form field, JSON response).
    #[must_use]
    pub fn encode(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.0)
    }

    /// Decode a wire-format CSRF token.
    ///
    /// # Errors
    ///
    /// - [`CsrfError::MalformedEncoding`] — input is not valid
    ///   base64-url-no-pad or decodes to a non-32-byte buffer.
    pub fn parse(encoded: &str) -> CsrfResult<Self> {
        let raw = URL_SAFE_NO_PAD
            .decode(encoded)
            .map_err(|_| CsrfError::MalformedEncoding)?;
        if raw.len() != TOKEN_LEN {
            return Err(CsrfError::MalformedEncoding);
        }
        let mut buf = [0u8; TOKEN_LEN];
        buf.copy_from_slice(&raw);
        Ok(Self(buf))
    }

    /// Constant-time comparison against a candidate token.
    /// `true` iff the two tokens are byte-equal.
    #[must_use]
    pub fn ct_eq(&self, other: &Self) -> bool {
        self.0.ct_eq(&other.0).into()
    }

    /// Borrowed view of the raw bytes — for the rare caller that
    /// needs to embed the token into a different envelope
    /// (e.g. signed cookie that carries the CSRF token alongside
    /// the session id).
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl PartialEq for CsrfToken {
    fn eq(&self, other: &Self) -> bool {
        self.ct_eq(other)
    }
}

impl Eq for CsrfToken {}

impl core::fmt::Debug for CsrfToken {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Never print the bytes — would defeat the
        // zero-on-drop guarantee for any logger that captures
        // `{token:?}`.
        f.debug_struct("CsrfToken")
            .field("len", &TOKEN_LEN)
            .finish_non_exhaustive()
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
    fn generate_yields_token() {
        let t = CsrfToken::generate().expect("ok");
        assert_eq!(t.as_bytes().len(), TOKEN_LEN);
    }

    #[test]
    fn generate_yields_distinct_tokens() {
        let a = CsrfToken::generate().expect("ok");
        let b = CsrfToken::generate().expect("ok");
        assert_ne!(a, b);
    }

    #[test]
    fn encode_then_parse_round_trips() {
        let t = CsrfToken::generate().expect("ok");
        let wire = t.encode();
        let parsed = CsrfToken::parse(&wire).expect("parse");
        assert!(t.ct_eq(&parsed));
    }

    #[test]
    fn encoded_length_is_43_chars() {
        // 32 bytes → base64-url-no-pad → 43 chars (32 * 4 / 3 = 42.66 → 43)
        let t = CsrfToken::generate().expect("ok");
        assert_eq!(t.encode().len(), 43);
    }

    #[test]
    fn encoded_value_is_url_safe() {
        let t = CsrfToken::generate().expect("ok");
        let wire = t.encode();
        for ch in wire.chars() {
            assert!(
                ch.is_ascii_alphanumeric() || ch == '-' || ch == '_',
                "non-url-safe char {ch:?}",
            );
        }
        assert!(!wire.contains('='));
    }

    #[test]
    fn parse_rejects_garbage() {
        let err = CsrfToken::parse("not base64!!").expect_err("bad");
        assert!(matches!(err, CsrfError::MalformedEncoding));
    }

    #[test]
    fn parse_rejects_too_short() {
        // 4 chars decode to 3 bytes — not 32.
        let err = CsrfToken::parse("AAAA").expect_err("short");
        assert!(matches!(err, CsrfError::MalformedEncoding));
    }

    #[test]
    fn parse_rejects_too_long() {
        let too_long: String = "A".repeat(60); // 45 bytes decoded
        let err = CsrfToken::parse(&too_long).expect_err("long");
        assert!(matches!(err, CsrfError::MalformedEncoding));
    }

    #[test]
    fn ct_eq_self_is_true() {
        let t = CsrfToken::generate().expect("ok");
        assert!(t.ct_eq(&t));
    }

    #[test]
    fn ct_eq_distinct_is_false() {
        let a = CsrfToken::generate().expect("ok");
        let b = CsrfToken::generate().expect("ok");
        assert!(!a.ct_eq(&b));
    }

    #[test]
    fn debug_does_not_print_bytes() {
        let t = CsrfToken::generate().expect("ok");
        let s = format!("{t:?}");
        assert!(s.contains("CsrfToken"));
        // The token has random bytes; printing them would leak.
        // Assert the debug string is the same shape every time:
        // it must NOT include any hex / base64 of the token.
        let wire = t.encode();
        assert!(!s.contains(&wire));
    }
}
