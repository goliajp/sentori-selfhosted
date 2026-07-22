//! Invite token: random 32 bytes on the wire, SHA-256 hash in DB.
//!
//! ## Wire format
//!
//! ```text
//! plaintext_token := base64url_no_pad(token_bytes)   // 43 ASCII chars
//! token_bytes      := 32 random bytes from OS CSPRNG
//! token_hash       := SHA-256(token_bytes)           // 32 bytes
//! ```
//!
//! Server stores `token_hash` only. A leaked database row cannot
//! be replayed against the invite-accept endpoint because the
//! one-way hash hides the cleartext token. The plaintext appears
//! in exactly two places in the server's process memory:
//!
//! 1. In [`MintedInvite::plaintext_token`], handed to the caller
//!    once. Caller must email it and drop it.
//! 2. In [`InviteToken::parse_and_hash`] when an invite-acceptance
//!    request arrives. The bytes are zeroed (via `zeroize`) on
//!    drop.
//!
//! ## Why SHA-256 with no salt?
//!
//! Per-token salts would require a second DB column and an
//! online lookup *before* the hash compare — defeats the
//! constant-time DB lookup story. Pre-image resistance of
//! SHA-256 on 32 random bytes (256-bit entropy) is the entire
//! security model; a salt buys nothing here. This is the same
//! reasoning the GitHub PAT, Slack token, and Linear API token
//! schemes use.

use std::fmt;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use zeroize::Zeroize;

use crate::error::IdentityError;
use crate::model::WorkspaceInvite;

/// Number of random bytes in an invite token (32).
///
/// 256 bits of entropy — overwhelmingly more than a brute-
/// forcer can scan the accept endpoint for. Wire-format encoded
/// as 43-char base64url-no-pad. Exposed publicly so callers
/// writing tokenless tests can construct synthetic-length
/// fixtures against the same constant.
pub const INVITE_TOKEN_BYTES: usize = 32;

/// SHA-256 hash of an invite token (32 bytes). Wraps the
/// fixed-size byte array so the public surface never
/// accidentally accepts a wrong-length slice.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct TokenHash(pub [u8; 32]);

impl TokenHash {
    /// Borrow as a byte slice for `BYTEA` parameter binding.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Constant-time equality check. Use this rather than `==`
    /// when comparing a request-derived hash against a stored
    /// one — though for `BYTEA` PK lookups Postgres already
    /// constant-times the index walk, so this matters mainly
    /// for unit tests and defence-in-depth.
    #[must_use]
    pub fn ct_eq(&self, other: &Self) -> bool {
        self.0.ct_eq(&other.0).into()
    }
}

impl fmt::Debug for TokenHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Hex-encode the hash for debug — short enough to print
        // and never sensitive (it's a hash, not the token).
        f.debug_tuple("TokenHash").field(&hex(&self.0)).finish()
    }
}

/// Plaintext invite token. Carries 32 random bytes on the wire
/// (base64url-no-pad on egress, raw bytes after parse).
///
/// `Drop` zeroes the buffer to keep the token out of process
/// memory after use.
pub struct InviteToken {
    bytes: [u8; INVITE_TOKEN_BYTES],
}

impl InviteToken {
    /// Mint a fresh random token from the OS CSPRNG.
    ///
    /// # Errors
    ///
    /// Returns [`IdentityError::Entropy`] if the OS RNG is
    /// unavailable. On modern Linux / macOS this is effectively
    /// impossible; it can fail on broken VMs where
    /// `/dev/urandom` is missing.
    pub fn generate() -> Result<Self, IdentityError> {
        let mut bytes = [0u8; INVITE_TOKEN_BYTES];
        getrandom::getrandom(&mut bytes).map_err(|e| IdentityError::Entropy(e.to_string()))?;
        Ok(Self { bytes })
    }

    /// Encode the token in its 43-char base64url-no-pad form
    /// for embedding in an email link or HTTP header.
    #[must_use]
    pub fn to_wire_string(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.bytes)
    }

    /// Parse a wire-format string and return the SHA-256 hash
    /// suitable for a DB lookup. The cleartext token is held in
    /// a stack buffer that is zeroed when this function
    /// returns — callers never see it.
    ///
    /// # Errors
    ///
    /// Returns [`IdentityError::InviteInvalid`] for any string
    /// that does not decode to exactly [`INVITE_TOKEN_BYTES`]
    /// bytes. The error variant is deliberately the same one
    /// returned by "token unknown" / "expired" / "accepted" —
    /// see [`IdentityError::InviteInvalid`] docs.
    pub fn parse_and_hash(s: &str) -> Result<TokenHash, IdentityError> {
        let mut decoded = URL_SAFE_NO_PAD
            .decode(s.as_bytes())
            .map_err(|_| IdentityError::InviteInvalid)?;
        if decoded.len() != INVITE_TOKEN_BYTES {
            decoded.zeroize();
            return Err(IdentityError::InviteInvalid);
        }
        let mut buf = [0u8; INVITE_TOKEN_BYTES];
        buf.copy_from_slice(&decoded);
        decoded.zeroize();
        let hash = sha256(&buf);
        buf.zeroize();
        Ok(TokenHash(hash))
    }

    /// SHA-256 hash of the token bytes. Used at mint time to
    /// stash the hash in the DB.
    #[must_use]
    pub fn hash(&self) -> TokenHash {
        TokenHash(sha256(&self.bytes))
    }
}

impl Drop for InviteToken {
    fn drop(&mut self) {
        self.bytes.zeroize();
    }
}

// Manual Debug — never reveal the plaintext token.
impl fmt::Debug for InviteToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InviteToken")
            .field("bytes", &"<redacted>")
            .finish()
    }
}

/// Return value of [`crate::Invites::create`].
///
/// Holds the persisted invite row alongside the freshly minted
/// plaintext token. The token is returned exactly once;
/// subsequent reads of the invite row yield only the SHA-256
/// hash.
#[derive(Debug)]
pub struct MintedInvite {
    /// The persisted invite row (no token).
    pub invite: WorkspaceInvite,
    /// Plaintext token, ready to embed in an email link as
    /// `https://app.example.com/invite/accept?token=<...>`.
    pub plaintext_token: InviteToken,
}

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
        // Hex writes are infallible on Strings.
        let _ = write!(s, "{b:02x}");
    }
    s
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_wire_string() {
        let token = InviteToken::generate().expect("rng");
        let wire = token.to_wire_string();
        assert_eq!(wire.len(), 43, "32 bytes base64url-no-pad = 43 chars");
        let parsed_hash = InviteToken::parse_and_hash(&wire).expect("parse");
        assert!(token.hash().ct_eq(&parsed_hash));
    }

    #[test]
    fn parse_rejects_wrong_length() {
        // 16 random bytes -> 22 chars; will decode but wrong length.
        let short = URL_SAFE_NO_PAD.encode([1u8; 16]);
        assert!(matches!(
            InviteToken::parse_and_hash(&short),
            Err(IdentityError::InviteInvalid)
        ));
    }

    #[test]
    fn parse_rejects_non_base64() {
        assert!(matches!(
            InviteToken::parse_and_hash("!@#$"),
            Err(IdentityError::InviteInvalid)
        ));
    }

    #[test]
    fn different_tokens_have_different_hashes() {
        let a = InviteToken::generate().expect("rng");
        let b = InviteToken::generate().expect("rng");
        assert!(!a.hash().ct_eq(&b.hash()));
    }

    #[test]
    fn token_debug_does_not_leak_bytes() {
        let token = InviteToken::generate().expect("rng");
        let debug = format!("{token:?}");
        assert!(debug.contains("redacted"));
        assert!(!debug.contains(&token.to_wire_string()));
    }
}
