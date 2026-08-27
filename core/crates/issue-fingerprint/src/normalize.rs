//! Message normalisation — replace dynamic identifiers with stable
//! tokens so they don't fragment grouping below the "same condition"
//! level.
//!
//! Two replacements run in a single pass, left-to-right:
//!
//! - **Digit runs** of length ≥ 4 collapse to `<N>`. Below 4 digits is
//!   often semantic (HTTP status codes, year ranges) so we leave them.
//! - **Canonical UUIDs** (`8-4-4-4-12` hex with dashes,
//!   case-insensitive) collapse to `<UUID>`. UUIDs are always a
//!   per-instance identifier in Sentori's use cases; collapsing them
//!   keeps `"failed for session 7f3..."` and `"failed for session
//!   c12..."` in one group.
//!
//! The implementation deliberately avoids regex: the patterns are
//! narrow, the hot path runs once per event, and pulling in
//! `regex` would balloon compile times and binary size for ~50 lines
//! of focused logic.

use std::borrow::Cow;

/// Minimum digit-run length collapsed to the `<N>` token.
///
/// `< 4` runs are usually semantic (HTTP status, year, small counts);
/// `>= 4` runs are almost always identifiers (user IDs, timestamps in
/// ms, request sequence numbers).
pub const MIN_DIGIT_RUN: usize = 4;

/// Replacement token used in place of a long digit run.
pub const DIGIT_TOKEN: &str = "<N>";

/// Replacement token used in place of a canonical UUID.
pub const UUID_TOKEN: &str = "<UUID>";

/// Normalise `msg` by collapsing dynamic identifiers to stable tokens.
///
/// Returns a [`Cow::Borrowed`] when nothing needed replacing — common
/// for short error messages without dynamic content — so the hot path
/// avoids an allocation.
///
/// ```rust
/// use sentori_issue_fingerprint::normalize;
///
/// assert_eq!(
///     normalize::message("User 12345 timed out"),
///     "User <N> timed out",
/// );
/// assert_eq!(
///     normalize::message("session 7f3b1c8a-2e3d-4f5a-9b0c-1d2e3f4a5b6c failed"),
///     "session <UUID> failed",
/// );
/// // No dynamic content → original message borrowed unchanged.
/// assert_eq!(normalize::message("TypeError: x is undefined"), "TypeError: x is undefined");
/// ```
#[must_use]
pub fn message(msg: &str) -> Cow<'_, str> {
    if !needs_normalisation(msg) {
        return Cow::Borrowed(msg);
    }

    let bytes = msg.as_bytes();
    let mut out = String::with_capacity(msg.len());
    let mut i = 0;
    while i < bytes.len() {
        // UUID first — it starts with hex and includes digits, so trying
        // it first means a UUID is recognised as a whole instead of
        // partially eaten by the digit-run pass.
        if let Some(end) = uuid_at(bytes, i) {
            out.push_str(UUID_TOKEN);
            i = end;
            continue;
        }

        if bytes[i].is_ascii_digit() {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            let run = j - i;
            if run >= MIN_DIGIT_RUN {
                out.push_str(DIGIT_TOKEN);
            } else {
                // Safe: `i..j` is an ASCII-digit slice, valid UTF-8.
                out.push_str(slice_str(msg, i, j));
            }
            i = j;
            continue;
        }

        // Copy one UTF-8 character verbatim. `msg` is `&str` so the
        // byte at `i` is a valid UTF-8 leading byte; advance by its
        // codepoint length.
        let ch_len = utf8_char_len(bytes[i]);
        out.push_str(slice_str(msg, i, i + ch_len));
        i += ch_len;
    }
    Cow::Owned(out)
}

/// Quick scan: does `msg` contain any pattern we'd rewrite?
///
/// Cheap to compute and lets the hot path stay borrow-only for
/// short, identifier-free messages.
fn needs_normalisation(msg: &str) -> bool {
    let bytes = msg.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Hex chars include digits, so the UUID probe must run BEFORE
        // the digit-run check — otherwise a UUID that happens to start
        // with `[0-9]` (about 38 % of all UUIDs) gets its first byte
        // eaten as a 1-char "digit run" and the probe fires from the
        // second byte where it cannot match.
        if bytes[i].is_ascii_hexdigit() && uuid_at(bytes, i).is_some() {
            return true;
        }
        if bytes[i].is_ascii_digit() {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j - i >= MIN_DIGIT_RUN {
                return true;
            }
            i = j;
            continue;
        }
        i += utf8_char_len(bytes[i]);
    }
    false
}

/// If `bytes[at..]` starts with a canonical `8-4-4-4-12` UUID, return
/// the exclusive end offset; otherwise return [`None`].
///
/// We accept lower- and upper-case hex (RFC 4122 is case-insensitive).
fn uuid_at(bytes: &[u8], at: usize) -> Option<usize> {
    // 8-4-4-4-12 hex with `-` separators is 36 bytes total.
    const SECTIONS: [usize; 5] = [8, 4, 4, 4, 12];
    let mut p = at;
    for (idx, &len) in SECTIONS.iter().enumerate() {
        if p + len > bytes.len() {
            return None;
        }
        for &b in &bytes[p..p + len] {
            if !b.is_ascii_hexdigit() {
                return None;
            }
        }
        p += len;
        if idx < SECTIONS.len() - 1 {
            if p >= bytes.len() || bytes[p] != b'-' {
                return None;
            }
            p += 1;
        }
    }
    Some(p)
}

/// UTF-8 code-point byte length for a leading byte.
///
/// Returns 1 for ASCII and continuation bytes (the latter should never
/// be the start of iteration in well-formed input, but defaulting to 1
/// keeps the walk forward).
const fn utf8_char_len(lead: u8) -> usize {
    match lead {
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF7 => 4,
        // ASCII (`0x00..=0x7F`) and continuation / invalid bytes
        // (`0x80..=0xBF`, `0xF8..=0xFF`) both advance one byte: the
        // former because they're 1-byte code points, the latter so
        // a stuck walk can't loop forever on malformed input.
        _ => 1,
    }
}

/// Inclusive-exclusive slice of `s` by byte offsets, returning a
/// `&str` view. Caller guarantees the offsets land on UTF-8
/// boundaries.
fn slice_str(s: &str, start: usize, end: usize) -> &str {
    // SAFETY argument: `start` and `end` are computed only at digit
    // boundaries, UUID boundaries (all ASCII), or `utf8_char_len`-aligned
    // code-point boundaries — all of which are valid `&str` slice
    // indices. Falling back to a non-slicing branch is not reachable
    // in practice.
    s.get(start..end).unwrap_or("")
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
    fn empty_message_borrows_unchanged() {
        let out = message("");
        assert!(matches!(out, Cow::Borrowed("")));
    }

    #[test]
    fn no_dynamic_content_borrows() {
        let out = message("TypeError: x is undefined");
        assert!(matches!(out, Cow::Borrowed(_)));
        assert_eq!(out, "TypeError: x is undefined");
    }

    #[test]
    fn three_digit_run_kept() {
        // HTTP statuses are semantic — keep them.
        assert_eq!(message("HTTP 404 not found"), "HTTP 404 not found");
    }

    #[test]
    fn four_digit_run_collapses() {
        assert_eq!(message("User 1234 timed out"), "User <N> timed out");
    }

    #[test]
    fn long_digit_run_collapses() {
        assert_eq!(message("ts=1733567890123 evt=ok"), "ts=<N> evt=ok",);
    }

    #[test]
    fn multiple_digit_runs_each_collapse() {
        assert_eq!(message("a 12345 b 67890 c"), "a <N> b <N> c",);
    }

    #[test]
    fn lowercase_uuid_collapses() {
        assert_eq!(
            message("session 7f3b1c8a-2e3d-4f5a-9b0c-1d2e3f4a5b6c failed"),
            "session <UUID> failed",
        );
    }

    #[test]
    fn uppercase_uuid_collapses() {
        assert_eq!(
            message("sid=7F3B1C8A-2E3D-4F5A-9B0C-1D2E3F4A5B6C"),
            "sid=<UUID>",
        );
    }

    #[test]
    fn malformed_uuid_section_lengths_left_alone() {
        // Wrong section widths (8-4-4-4-11) — not a real UUID.
        let msg = "x 7f3b1c8a-2e3d-4f5a-9b0c-1d2e3f4a5b6 y";
        let out = message(msg);
        assert!(!out.contains(UUID_TOKEN), "{out}");
    }

    #[test]
    fn uuid_at_end_of_string() {
        assert_eq!(
            message("sid 11111111-2222-3333-4444-555555555555"),
            "sid <UUID>",
        );
    }

    #[test]
    fn unicode_message_round_trips_when_no_match() {
        // 多字节字符串无 dynamic 部分 → 原样借用。
        let s = "ユーザがタイムアウトしました";
        let out = message(s);
        assert!(matches!(out, Cow::Borrowed(_)));
        assert_eq!(out, s);
    }

    #[test]
    fn unicode_with_digit_run_still_normalises() {
        assert_eq!(
            message("ユーザ 12345 タイムアウト"),
            "ユーザ <N> タイムアウト",
        );
    }

    #[test]
    fn back_to_back_uuid_then_digits() {
        assert_eq!(
            message("a 11111111-2222-3333-4444-555555555555 1234"),
            "a <UUID> <N>",
        );
    }

    #[test]
    fn three_digits_then_uuid() {
        assert_eq!(
            message("err 404 sid=11111111-2222-3333-4444-555555555555"),
            "err 404 sid=<UUID>",
        );
    }

    #[test]
    fn uuid_missing_dash_left_alone() {
        // Even hex but no dashes — not a canonical UUID.
        let msg = "x 7f3b1c8a2e3d4f5a9b0c1d2e3f4a5b6c y";
        let out = message(msg);
        // Whole hex run is 32 chars — that's a long digit/hex blob, but
        // not all digits, so digit-run pass doesn't touch it either.
        assert!(!out.contains(UUID_TOKEN), "{out}");
    }

    #[test]
    fn utf8_char_len_covers_known_leads() {
        assert_eq!(utf8_char_len(b'a'), 1);
        assert_eq!(utf8_char_len(0xC3), 2);
        assert_eq!(utf8_char_len(0xE3), 3);
        assert_eq!(utf8_char_len(0xF0), 4);
        // Continuation / invalid bytes fall through to 1, keeping the
        // walk forward instead of getting stuck.
        assert_eq!(utf8_char_len(0x80), 1);
        assert_eq!(utf8_char_len(0xFF), 1);
    }

    #[test]
    fn uuid_at_rejects_at_buffer_edge() {
        // Buffer too short to even fit the first 8-hex section.
        assert_eq!(uuid_at(b"abcdef", 0), None);
    }
}
