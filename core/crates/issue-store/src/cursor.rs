//! Keyset-pagination [`Cursor`].
//!
//! Wire form is opaque base64url-no-pad: the dashboard hands
//! it back and forth without inspecting. Internally it
//! carries `(timestamp_unix_micros: i64, id: Uuid, limit: u32)`
//! so the SQL `WHERE (last_seen, id) < (cursor.ts, cursor.id)
//! ORDER BY last_seen DESC, id DESC LIMIT cursor.limit` can be
//! built with one parameter pair regardless of which entity
//! is being paged (issues by `last_seen`, events by
//! `timestamp`).
//!
//! The `(ts, id)` pair makes the sort key strictly
//! monotonic — issues that share a `last_seen` tie-break by
//! id, so the cursor walk doesn't skip rows or double-yield.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

/// Maximum page size any cursor may request. Defends against
/// the dashboard accidentally asking for 100k rows.
pub const CURSOR_MAX_LIMIT: u32 = 500;

/// Minimum page size — 1 row per call still works, anything
/// less is a no-op.
pub const CURSOR_MIN_LIMIT: u32 = 1;

/// Default page size when the caller invokes [`Cursor::start`].
pub const CURSOR_DEFAULT_LIMIT: u32 = 50;

/// Opaque keyset pagination cursor.
///
/// Construct with [`Cursor::start`] for the first page (which
/// has no `(ts, id)` filter — fetches the newest N rows).
/// Subsequent pages use [`Cursor::next`] (produced by
/// [`crate::PaginatedIssues::next`] / [`crate::PaginatedEvents::next`]).
///
/// `to_wire_string` produces the base64 form to ship to the
/// client; `parse` round-trips it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Cursor {
    /// `None` for the first page; `Some((timestamp, id))` for
    /// subsequent pages — the SQL filter is then
    /// `(sort_key, id) < (timestamp, id)`.
    pub anchor: Option<(OffsetDateTime, Uuid)>,
    /// Page size. Clamped to [`CURSOR_MIN_LIMIT`]..=[`CURSOR_MAX_LIMIT`]
    /// at parse and at construction.
    pub limit: u32,
}

impl Cursor {
    /// First-page cursor with the given `limit`.
    ///
    /// `limit` is clamped to the valid range — pass anything;
    /// `0` becomes [`CURSOR_MIN_LIMIT`], > 500 becomes
    /// [`CURSOR_MAX_LIMIT`].
    #[must_use]
    pub fn start(limit: u32) -> Self {
        Self {
            anchor: None,
            limit: limit.clamp(CURSOR_MIN_LIMIT, CURSOR_MAX_LIMIT),
        }
    }

    /// Next-page cursor anchored at `(timestamp, id)`.
    ///
    /// `limit` is clamped to the valid range.
    #[must_use]
    pub fn next(timestamp: OffsetDateTime, id: Uuid, limit: u32) -> Self {
        Self {
            anchor: Some((timestamp, id)),
            limit: limit.clamp(CURSOR_MIN_LIMIT, CURSOR_MAX_LIMIT),
        }
    }

    /// Encode to the 28-or-44-char wire form.
    ///
    /// First-page cursors encode as `"_:<limit>"` (4-6 chars);
    /// anchored cursors encode the full
    /// `(ts_micros: i64 LE)(id: 16 bytes)(limit: u32 LE)`
    /// = 28 bytes → 38 base64url-no-pad chars.
    #[must_use]
    pub fn to_wire_string(&self) -> String {
        let mut buf = Vec::with_capacity(28);
        match self.anchor {
            None => {
                // tag byte 0 = first page; payload = limit only.
                buf.push(0u8);
                buf.extend_from_slice(&self.limit.to_le_bytes());
            }
            Some((ts, id)) => {
                // tag byte 1 = anchored.
                buf.push(1u8);
                let micros = ts.unix_timestamp_nanos() / 1000; // i128 → i64 fits for any sane date
                let micros_i64 = i64::try_from(micros).unwrap_or(i64::MAX);
                buf.extend_from_slice(&micros_i64.to_le_bytes());
                buf.extend_from_slice(id.as_bytes());
                buf.extend_from_slice(&self.limit.to_le_bytes());
            }
        }
        URL_SAFE_NO_PAD.encode(&buf)
    }

    /// Parse a wire-format cursor string.
    ///
    /// # Errors
    ///
    /// [`CursorParseError`] for malformed input. Limit values
    /// out of range are clamped silently — only structural
    /// errors surface.
    pub fn parse(s: &str) -> Result<Self, CursorParseError> {
        let bytes = URL_SAFE_NO_PAD
            .decode(s.as_bytes())
            .map_err(|_| CursorParseError::Base64)?;
        match bytes.first().copied() {
            Some(0) if bytes.len() == 1 + 4 => {
                let limit = u32::from_le_bytes(bytes[1..5].try_into().unwrap_or([0; 4]));
                Ok(Self::start(limit))
            }
            Some(1) if bytes.len() == 1 + 8 + 16 + 4 => {
                let micros = i64::from_le_bytes(bytes[1..9].try_into().unwrap_or([0; 8]));
                let nanos = i128::from(micros) * 1000;
                let ts = OffsetDateTime::from_unix_timestamp_nanos(nanos)
                    .map_err(|_| CursorParseError::TimestampOutOfRange)?;
                let mut id_bytes = [0u8; 16];
                id_bytes.copy_from_slice(&bytes[9..25]);
                let id = Uuid::from_bytes(id_bytes);
                let limit = u32::from_le_bytes(bytes[25..29].try_into().unwrap_or([0; 4]));
                Ok(Self::next(ts, id, limit))
            }
            _ => Err(CursorParseError::WrongShape),
        }
    }
}

impl Default for Cursor {
    /// Default cursor: first page, 50 rows.
    fn default() -> Self {
        Self::start(CURSOR_DEFAULT_LIMIT)
    }
}

/// Errors from [`Cursor::parse`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CursorParseError {
    /// Base64 decode failed.
    #[error("cursor is not valid base64url")]
    Base64,
    /// Decoded payload had the wrong byte count for either
    /// the first-page or anchored shape.
    #[error("cursor payload has unexpected shape")]
    WrongShape,
    /// Timestamp deserialized to a value outside `time` crate's
    /// representable range.
    #[error("cursor timestamp is out of range")]
    TimestampOutOfRange,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn start_clamps_limit() {
        assert_eq!(Cursor::start(0).limit, CURSOR_MIN_LIMIT);
        assert_eq!(Cursor::start(10_000).limit, CURSOR_MAX_LIMIT);
        assert_eq!(Cursor::start(50).limit, 50);
    }

    #[test]
    fn round_trip_first_page() {
        let c = Cursor::start(50);
        let s = c.to_wire_string();
        let back = Cursor::parse(&s).unwrap();
        assert_eq!(back, c);
    }

    #[test]
    fn round_trip_anchored() {
        let ts = OffsetDateTime::from_unix_timestamp(1_767_225_600).unwrap();
        let id = Uuid::from_bytes([0x11; 16]);
        let c = Cursor::next(ts, id, 25);
        let s = c.to_wire_string();
        let back = Cursor::parse(&s).unwrap();
        assert_eq!(back, c);
    }

    #[test]
    fn rejects_garbage() {
        assert!(matches!(
            Cursor::parse("!!!").unwrap_err(),
            CursorParseError::Base64
        ));
        assert!(matches!(
            Cursor::parse("AA").unwrap_err(),
            CursorParseError::WrongShape
        ));
    }

    #[test]
    fn default_is_first_page_default_limit() {
        let c = Cursor::default();
        assert!(c.anchor.is_none());
        assert_eq!(c.limit, CURSOR_DEFAULT_LIMIT);
    }
}
