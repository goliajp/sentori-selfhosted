//! Token wire-format parse + hash.

use sha2::{Digest, Sha256};

/// Required wire-format prefix.
pub const TOKEN_PREFIX: &str = "st_pk_";

/// Wire-format value length (base32 chars after prefix).
pub const TOKEN_VALUE_LEN: usize = 26;

/// SHA-256 hash of the full token string (prefix included),
/// hex-encoded so it fits the `tokens.token_hash TEXT` column
/// per migration 0016. Legacy schema also stored hex text.
#[must_use]
pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

/// Lightweight syntactic check — full validation happens at
/// DB lookup. Returns true if the token starts with `st_pk_`
/// and has at least the expected length.
#[must_use]
pub fn looks_like_token(token: &str) -> bool {
    token.starts_with(TOKEN_PREFIX) && token.len() >= TOKEN_PREFIX.len() + TOKEN_VALUE_LEN
}
