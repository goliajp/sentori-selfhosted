//! [`BlobHash`] — SHA-256 newtype that doubles as the store key.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// SHA-256 digest length in bytes (32 = 256 bits).
pub const BLOB_HASH_BYTES: usize = 32;

/// Content-addressed blob key.
///
/// Wire form is the 64-char lowercase hex string of the
/// SHA-256 digest. Serialised the same way in JSON via
/// [`serde::Serialize`] / [`serde::Deserialize`].
///
/// The newtype prevents callers from accidentally passing a
/// non-hash `[u8; 32]` (e.g. a session-id hash) into the store.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlobHash([u8; BLOB_HASH_BYTES]);

impl BlobHash {
    /// Compute the hash of `bytes`.
    #[must_use]
    pub fn of(bytes: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let digest = hasher.finalize();
        let mut out = [0u8; BLOB_HASH_BYTES];
        out.copy_from_slice(&digest);
        Self(out)
    }

    /// Wrap a raw `[u8; 32]` (e.g. round-tripped through a DB
    /// `BYTEA` column).
    #[must_use]
    pub const fn from_bytes(bytes: [u8; BLOB_HASH_BYTES]) -> Self {
        Self(bytes)
    }

    /// Borrow the raw 32 bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; BLOB_HASH_BYTES] {
        &self.0
    }

    /// Consume into the raw 32-byte array.
    #[must_use]
    pub const fn into_bytes(self) -> [u8; BLOB_HASH_BYTES] {
        self.0
    }

    /// Hex-encode to the 64-char lowercase string. Stable wire
    /// format — round-trips via [`BlobHash::from_hex`].
    #[must_use]
    pub fn to_hex(&self) -> String {
        let mut s = String::with_capacity(BLOB_HASH_BYTES * 2);
        for b in &self.0 {
            use std::fmt::Write as _;
            // hex writes are infallible on a String.
            let _ = write!(s, "{b:02x}");
        }
        s
    }

    /// Parse from a 64-char hex string.
    ///
    /// Accepts upper or lower case hex. Returns
    /// [`BlobHashParseError`] for any length other than 64 or
    /// any character outside `[0-9a-fA-F]`.
    ///
    /// # Errors
    ///
    /// See [`BlobHashParseError`].
    pub fn from_hex(s: &str) -> Result<Self, BlobHashParseError> {
        if s.len() != BLOB_HASH_BYTES * 2 {
            return Err(BlobHashParseError::WrongLength {
                expected: BLOB_HASH_BYTES * 2,
                got: s.len(),
            });
        }
        let mut out = [0u8; BLOB_HASH_BYTES];
        for (i, chunk) in s.as_bytes().chunks_exact(2).enumerate() {
            let hi = decode_nibble(chunk[0])?;
            let lo = decode_nibble(chunk[1])?;
            out[i] = (hi << 4) | lo;
        }
        Ok(Self(out))
    }
}

impl fmt::Debug for BlobHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("BlobHash").field(&self.to_hex()).finish()
    }
}

impl fmt::Display for BlobHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl FromStr for BlobHash {
    type Err = BlobHashParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_hex(s)
    }
}

impl Serialize for BlobHash {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for BlobHash {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::from_hex(&s).map_err(serde::de::Error::custom)
    }
}

/// Errors from [`BlobHash::from_hex`] / [`BlobHash::from_str`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum BlobHashParseError {
    /// String wasn't 64 characters long.
    #[error("blob hash must be {expected} chars, got {got}")]
    WrongLength {
        /// Required length (64).
        expected: usize,
        /// Length supplied by the caller.
        got: usize,
    },

    /// String contained a non-hex character.
    #[error("blob hash contains non-hex character: {0:?}")]
    NonHexCharacter(char),
}

const fn decode_nibble(b: u8) -> Result<u8, BlobHashParseError> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(BlobHashParseError::NonHexCharacter(b as char)),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn known_vector_empty_input() {
        // SHA-256 of empty string is well-known.
        let h = BlobHash::of(b"");
        assert_eq!(
            h.to_hex(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn known_vector_abc() {
        let h = BlobHash::of(b"abc");
        assert_eq!(
            h.to_hex(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn round_trip_hex() {
        let h = BlobHash::of(b"hello world");
        let s = h.to_hex();
        let back = BlobHash::from_hex(&s).expect("parse");
        assert_eq!(back, h);
    }

    #[test]
    fn round_trip_serde() {
        let h = BlobHash::of(b"hello world");
        let json = serde_json::to_string(&h).expect("ser");
        // Hex is wrapped in double quotes.
        assert_eq!(json, format!("\"{}\"", h.to_hex()));
        let back: BlobHash = serde_json::from_str(&json).expect("de");
        assert_eq!(back, h);
    }

    #[test]
    fn case_insensitive_parse() {
        let lower = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        let upper = lower.to_ascii_uppercase();
        assert_eq!(
            BlobHash::from_hex(lower).unwrap(),
            BlobHash::from_hex(&upper).unwrap()
        );
    }

    #[test]
    fn rejects_wrong_length() {
        let err = BlobHash::from_hex("ab").expect_err("too short");
        assert!(matches!(err, BlobHashParseError::WrongLength { .. }));
        let too_long = "a".repeat(65);
        let err = BlobHash::from_hex(&too_long).expect_err("too long");
        assert!(matches!(err, BlobHashParseError::WrongLength { .. }));
    }

    #[test]
    fn rejects_non_hex_char() {
        let bad = format!("z{}", "a".repeat(63));
        let err = BlobHash::from_hex(&bad).expect_err("bad char");
        assert!(matches!(err, BlobHashParseError::NonHexCharacter('z')));
    }

    #[test]
    fn debug_includes_hex() {
        let h = BlobHash::of(b"x");
        let s = format!("{h:?}");
        assert!(s.contains(&h.to_hex()));
    }

    #[test]
    fn display_is_hex() {
        let h = BlobHash::of(b"x");
        assert_eq!(format!("{h}"), h.to_hex());
    }

    #[test]
    fn fromstr_matches_from_hex() {
        let s = BlobHash::of(b"q").to_hex();
        let a: BlobHash = s.parse().expect("FromStr");
        let b = BlobHash::from_hex(&s).expect("from_hex");
        assert_eq!(a, b);
    }

    #[test]
    fn distinct_inputs_distinct_hashes() {
        assert_ne!(BlobHash::of(b"a"), BlobHash::of(b"b"));
    }

    #[test]
    fn same_input_same_hash() {
        assert_eq!(BlobHash::of(b"hello"), BlobHash::of(b"hello"));
    }
}
