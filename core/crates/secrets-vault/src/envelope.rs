//! On-the-wire envelope format + (de)serialization.
//!
//! ## Layout (version `0x01`)
//!
//! ```text
//!   offset  bytes  field
//!   ──────  ─────  ─────
//!   0       1      version (always 0x01 today)
//!   1       1      key_id_len (1..=255)
//!   2       N      key_id bytes (printable ASCII)
//!   2+N     12     wrapped_dek_nonce
//!   14+N    48     wrapped_dek (32-byte DEK + 16-byte GCM tag)
//!   62+N    12     payload_nonce
//!   74+N    ..     payload_ciphertext (plaintext bytes + 16-byte
//!                  GCM tag appended by aes-gcm)
//! ```
//!
//! Total fixed overhead: `74 + key_id_len + 16 (payload tag) =
//! 90 + key_id_len bytes`.

use crate::error::OpenError;

/// Current envelope version.
pub(crate) const VERSION_V1: u8 = 0x01;

/// AES-GCM nonce length (fixed by the construction).
pub(crate) const NONCE_LEN: usize = 12;

/// AES-GCM authentication tag length (fixed by Aes256Gcm).
pub(crate) const TAG_LEN: usize = 16;

/// Length of a wrapped DEK: 32-byte DEK + 16-byte tag.
pub(crate) const WRAPPED_DEK_LEN: usize = 32 + TAG_LEN;

/// Minimum sealed envelope length: version (1) + key_id_len (1) +
/// 1-byte key_id + wrapped_dek_nonce + wrapped_dek + payload_nonce
/// + payload_tag = 1 + 1 + 1 + 12 + 48 + 12 + 16 = 91 bytes.
pub(crate) const MIN_ENVELOPE_LEN: usize =
    1 + 1 + 1 + NONCE_LEN + WRAPPED_DEK_LEN + NONCE_LEN + TAG_LEN;

/// Borrowed view of a parsed envelope. Fields point into the
/// caller's buffer.
#[derive(Debug)]
pub(crate) struct EnvelopeView<'a> {
    pub key_id: &'a [u8],
    pub wrapped_dek_nonce: &'a [u8; NONCE_LEN],
    pub wrapped_dek: &'a [u8; WRAPPED_DEK_LEN],
    pub payload_nonce: &'a [u8; NONCE_LEN],
    pub payload_ciphertext: &'a [u8],
}

/// Parse an envelope. Validates version, length, and key_id_len
/// only — the AEAD tags are validated downstream during decrypt.
pub(crate) fn parse(buf: &[u8]) -> Result<EnvelopeView<'_>, OpenError> {
    if buf.len() < MIN_ENVELOPE_LEN {
        return Err(OpenError::Truncated);
    }
    let version = buf[0];
    if version != VERSION_V1 {
        return Err(OpenError::UnsupportedVersion(version));
    }
    let key_id_len = buf[1] as usize;
    if key_id_len == 0 {
        return Err(OpenError::InvalidKeyIdLength);
    }
    // Compute total required length now we know key_id_len.
    let required = 2 + key_id_len + NONCE_LEN + WRAPPED_DEK_LEN + NONCE_LEN + TAG_LEN;
    if buf.len() < required {
        return Err(OpenError::InvalidKeyIdLength);
    }

    let mut cursor = 2;
    let key_id = &buf[cursor..cursor + key_id_len];
    cursor += key_id_len;

    let wrapped_dek_nonce: &[u8; NONCE_LEN] = buf[cursor..cursor + NONCE_LEN]
        .try_into()
        .map_err(|_| OpenError::Truncated)?;
    cursor += NONCE_LEN;

    let wrapped_dek: &[u8; WRAPPED_DEK_LEN] = buf[cursor..cursor + WRAPPED_DEK_LEN]
        .try_into()
        .map_err(|_| OpenError::Truncated)?;
    cursor += WRAPPED_DEK_LEN;

    let payload_nonce: &[u8; NONCE_LEN] = buf[cursor..cursor + NONCE_LEN]
        .try_into()
        .map_err(|_| OpenError::Truncated)?;
    cursor += NONCE_LEN;

    let payload_ciphertext = &buf[cursor..];
    debug_assert!(payload_ciphertext.len() >= TAG_LEN);

    Ok(EnvelopeView {
        key_id,
        wrapped_dek_nonce,
        wrapped_dek,
        payload_nonce,
        payload_ciphertext,
    })
}

/// Serialise the envelope fields into `out`. Caller pre-allocates.
pub(crate) fn serialise(
    out: &mut Vec<u8>,
    key_id: &[u8],
    wrapped_dek_nonce: &[u8; NONCE_LEN],
    wrapped_dek: &[u8],
    payload_nonce: &[u8; NONCE_LEN],
    payload_ciphertext: &[u8],
) {
    out.push(VERSION_V1);
    // Caller has already validated key_id length via KeyId::new.
    out.push(u8::try_from(key_id.len()).unwrap_or(u8::MAX));
    out.extend_from_slice(key_id);
    out.extend_from_slice(wrapped_dek_nonce);
    out.extend_from_slice(wrapped_dek);
    out.extend_from_slice(payload_nonce);
    out.extend_from_slice(payload_ciphertext);
}

/// Peek at a sealed blob's key id without unwrapping anything.
/// Returns `None` on a malformed envelope. Use to drive rotation
/// (try current vault; on mismatch, look up the legacy vault by
/// the peeked id).
#[must_use]
pub fn peek_key_id(sealed: &[u8]) -> Option<&str> {
    let view = parse(sealed).ok()?;
    core::str::from_utf8(view.key_id).ok()
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

    fn build_envelope(key_id: &[u8], payload_len: usize) -> Vec<u8> {
        let mut out = Vec::new();
        serialise(
            &mut out,
            key_id,
            &[0xAA; NONCE_LEN],
            &[0xBB; WRAPPED_DEK_LEN],
            &[0xCC; NONCE_LEN],
            &vec![0xDD; payload_len + TAG_LEN],
        );
        out
    }

    #[test]
    fn parse_round_trips() {
        let buf = build_envelope(b"master-v1", 64);
        let view = parse(&buf).expect("parse");
        assert_eq!(view.key_id, b"master-v1");
        assert_eq!(view.wrapped_dek_nonce, &[0xAA; NONCE_LEN]);
        assert_eq!(view.wrapped_dek, &[0xBB; WRAPPED_DEK_LEN]);
        assert_eq!(view.payload_nonce, &[0xCC; NONCE_LEN]);
        assert_eq!(view.payload_ciphertext.len(), 64 + TAG_LEN);
    }

    #[test]
    fn parse_rejects_truncated() {
        let buf = build_envelope(b"master-v1", 0);
        // Drop the last 5 bytes — buffer is now shorter than the
        // claimed key_id_len requires.
        let truncated = &buf[..buf.len() - 5];
        let err = parse(truncated).expect_err("truncated");
        assert!(matches!(err, OpenError::InvalidKeyIdLength));
    }

    #[test]
    fn parse_rejects_below_minimum_length() {
        let err = parse(&[0; 4]).expect_err("min");
        assert!(matches!(err, OpenError::Truncated));
    }

    #[test]
    fn parse_rejects_unsupported_version() {
        let mut buf = build_envelope(b"id", 0);
        buf[0] = 0xff;
        let err = parse(&buf).expect_err("version");
        assert!(matches!(err, OpenError::UnsupportedVersion(0xff)));
    }

    #[test]
    fn parse_rejects_zero_key_id_len() {
        let mut buf = build_envelope(b"id", 0);
        buf[1] = 0;
        let err = parse(&buf).expect_err("zero kid");
        assert!(matches!(err, OpenError::InvalidKeyIdLength));
    }

    #[test]
    fn peek_key_id_returns_str_on_valid_envelope() {
        let buf = build_envelope(b"master-v1", 8);
        assert_eq!(peek_key_id(&buf), Some("master-v1"));
    }

    #[test]
    fn peek_key_id_returns_none_on_garbage() {
        assert!(peek_key_id(&[1, 2, 3]).is_none());
    }

    #[test]
    fn min_envelope_len_matches_layout() {
        // 1 (version) + 1 (key_id_len) + 1 (smallest key_id) +
        // 12 (wrapped_dek_nonce) + 48 (wrapped_dek) +
        // 12 (payload_nonce) + 16 (payload_tag) = 91.
        assert_eq!(MIN_ENVELOPE_LEN, 91);
    }
}
