//! [`Scrubber`] — PII redaction over NDJSON wireframe lines.
//!
//! Walks each NDJSON line, parses to `serde_json::Value`,
//! recursively replaces any string field's contents whose
//! match against the configured pattern list yields ≥ 1 hit.
//! The replacement is [`REDACTED_PLACEHOLDER`] surrounded by
//! the un-matched prefix / suffix — i.e. `"email: alice@x.com,
//! ok"` → `"email: [REDACTED], ok"`.
//!
//! Built-in OWASP patterns cover email, US phone, credit-card
//! (Luhn-ish — no full Luhn check; matches the standard
//! 13-19 digit / dash / space format used in form fields),
//! and US SSN (`XXX-XX-XXXX`). The set is opinionated towards
//! "what the dashboard's PII tagger would catch"; per-instance
//! [`Scrubber::with_extra`] adds project-specific patterns
//! (employee id format, internal endpoint URL, etc).
//!
//! The pipeline counts redactions for the
//! [`crate::ReplaySession::scrubbed_count`] dashboard badge.

use std::io::{BufRead, BufReader, Write};

use regex::Regex;
use serde_json::Value;
use thiserror::Error;

/// String that replaces matched PII substrings.
pub const REDACTED_PLACEHOLDER: &str = "[REDACTED]";

/// OWASP-flavoured PII regex pack.
///
/// Patterns are deliberately permissive to favour
/// false-positive scrubs over false-negative leaks (the
/// downside of redacting "alice@example.com" twice is zero;
/// the downside of leaking it once is real).
fn owasp_pack() -> Result<Vec<Regex>, regex::Error> {
    Ok(vec![
        // RFC 5322-ish email.
        Regex::new(r"\b[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}\b")?,
        // US 10-digit phone: (xxx) xxx-xxxx | xxx-xxx-xxxx |
        // +1 xxx xxx xxxx — minimal "looks like a phone".
        Regex::new(
            r"(?x)
            (?:\+?1[\s.\-]?)?
            \(?\d{3}\)?[\s.\-]?
            \d{3}[\s.\-]?
            \d{4}
        ",
        )?,
        // Credit card 13–19 digits, optionally dash/space
        // grouped (Visa / MC / AmEx / Discover / JCB shapes).
        Regex::new(r"\b(?:\d[\s\-]?){12,18}\d\b")?,
        // US SSN.
        Regex::new(r"\b\d{3}-\d{2}-\d{4}\b")?,
    ])
}

/// PII scrubber. Built once at startup; cheap to clone (each
/// `Regex` is `Arc`-shared internally by the `regex` crate).
#[derive(Debug, Clone)]
pub struct Scrubber {
    patterns: Vec<Regex>,
}

impl Scrubber {
    /// Build a scrubber with the OWASP pack pre-loaded.
    ///
    /// # Panics
    ///
    /// Never — every pattern in the OWASP pack is a hardcoded
    /// literal known to compile. The fallible API is kept for
    /// future patterns that might be config-driven.
    #[must_use]
    pub fn owasp_default() -> Self {
        Self {
            patterns: owasp_pack().expect("owasp pack must compile"),
        }
    }

    /// Build an empty scrubber. Call [`Self::with_extra`] to
    /// stack patterns.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            patterns: Vec::new(),
        }
    }

    /// Add one more compiled regex (builder-style).
    #[must_use]
    pub fn with_extra(mut self, pattern: Regex) -> Self {
        self.patterns.push(pattern);
        self
    }

    /// Compile and add a regex pattern.
    ///
    /// # Errors
    ///
    /// [`ScrubberError::InvalidPattern`] if the regex fails to
    /// compile.
    pub fn try_with_extra(mut self, pattern: &str) -> Result<Self, ScrubberError> {
        let re = Regex::new(pattern).map_err(|e| ScrubberError::InvalidPattern {
            pattern: pattern.to_string(),
            source: e,
        })?;
        self.patterns.push(re);
        Ok(self)
    }

    /// Number of patterns configured.
    #[must_use]
    pub const fn pattern_count(&self) -> usize {
        self.patterns.len()
    }

    /// Scrub one NDJSON byte buffer.
    ///
    /// Returns `(scrubbed_bytes, report)`. The report counts
    /// `frame_count` (lines processed) + `redaction_count`
    /// (per-field substring hits across all lines + nested
    /// values).
    ///
    /// Empty / whitespace-only lines pass through unchanged.
    /// Malformed JSON lines fail-open — they're kept verbatim
    /// and counted in `report.malformed_lines`; scrubber
    /// doesn't reject the whole batch on one bad line.
    ///
    /// # Errors
    ///
    /// [`ScrubberError::Io`] only on stdlib `Write` failure
    /// (can't happen against `Vec<u8>`). The fallible API
    /// keeps the door open for future streaming `impl Write`
    /// outputs.
    pub fn scrub(&self, ndjson: &[u8]) -> Result<(Vec<u8>, ScrubReport), ScrubberError> {
        let mut out = Vec::with_capacity(ndjson.len());
        let reader = BufReader::new(ndjson);
        let mut report = ScrubReport::default();
        for (line_idx, line) in reader.split(b'\n').enumerate() {
            let line = line.map_err(ScrubberError::Io)?;
            if line.iter().all(u8::is_ascii_whitespace) {
                // Empty / blank — drop trailing on rebuild so
                // we don't get \n\n. (Caller's input may have
                // trailing newline; we still emit a \n after
                // every non-trailing line below.)
                if !line.is_empty() {
                    out.extend_from_slice(&line);
                    out.push(b'\n');
                }
                continue;
            }
            report.frame_count += 1;

            let parsed: Result<Value, _> = serde_json::from_slice(&line);
            let Ok(mut value) = parsed else {
                tracing::warn!(line_idx, "scrub: malformed JSON; passing through");
                report.malformed_lines += 1;
                out.extend_from_slice(&line);
                out.push(b'\n');
                continue;
            };

            let hits = scrub_value(&mut value, &self.patterns);
            report.redaction_count += hits;

            out.write_all(value.to_string().as_bytes())
                .map_err(ScrubberError::Io)?;
            out.push(b'\n');
        }
        // Trim final newline if the input didn't have one — we
        // always emit one per non-empty line, so over-emit on a
        // single-line input without trailing \n.
        if !ndjson.ends_with(b"\n") && out.ends_with(b"\n") {
            out.pop();
        }
        Ok((out, report))
    }
}

impl Default for Scrubber {
    fn default() -> Self {
        Self::owasp_default()
    }
}

/// Per-call scrub statistics.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ScrubReport {
    /// Number of NDJSON lines processed (excludes blanks).
    pub frame_count: i32,
    /// Total substring matches replaced across all lines.
    pub redaction_count: i32,
    /// Lines that failed JSON parse and were passed through
    /// verbatim.
    pub malformed_lines: i32,
}

/// Walk `value` recursively; replace any string content
/// whose match against any pattern produces ≥ 1 hit. Returns
/// the number of pattern hits (one per non-overlapping match).
fn scrub_value(value: &mut Value, patterns: &[Regex]) -> i32 {
    let mut hits = 0i32;
    match value {
        Value::String(s) => {
            for pat in patterns {
                // `replace_all` returns Cow::Borrowed when no
                // match — counts let us detect actual hits
                // without an extra `find_iter` walk.
                let match_count = pat.find_iter(s).count();
                if match_count > 0 {
                    *s = pat.replace_all(s, REDACTED_PLACEHOLDER).into_owned();
                    hits += match_count as i32;
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                hits += scrub_value(item, patterns);
            }
        }
        Value::Object(map) => {
            for (_k, v) in map.iter_mut() {
                hits += scrub_value(v, patterns);
            }
        }
        // Null / Bool / Number are PII-free.
        _ => {}
    }
    hits
}

/// Errors from the scrubber.
#[derive(Debug, Error)]
pub enum ScrubberError {
    /// I/O while reading/writing the NDJSON buffer. Practically
    /// unreachable against `Vec<u8>`.
    #[error("scrubber I/O: {0}")]
    Io(#[from] std::io::Error),

    /// `try_with_extra` rejected an uncompilable pattern.
    #[error("invalid scrubber pattern {pattern:?}: {source}")]
    InvalidPattern {
        /// The pattern that failed to compile.
        pattern: String,
        /// The compile error.
        #[source]
        source: regex::Error,
    },
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn redacts_email_in_text_field() {
        let s = Scrubber::owasp_default();
        let ndjson = br#"{"text":"contact alice@example.com please"}"#;
        let (out, report) = s.scrub(ndjson).unwrap();
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("[REDACTED]"));
        assert!(!s.contains("alice@example.com"));
        assert_eq!(report.redaction_count, 1);
        assert_eq!(report.frame_count, 1);
    }

    #[test]
    fn redacts_inside_array_and_object() {
        let s = Scrubber::owasp_default();
        let ndjson = br#"{"nodes":[{"text":"email a@b.co"},{"text":"clean"}]}"#;
        let (out, report) = s.scrub(ndjson).unwrap();
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("[REDACTED]"));
        assert!(s.contains("clean"));
        assert_eq!(report.redaction_count, 1);
    }

    #[test]
    fn non_text_unchanged() {
        let s = Scrubber::owasp_default();
        let ndjson = br#"{"x":10,"y":20,"text":"clean"}"#;
        let (out, report) = s.scrub(ndjson).unwrap();
        assert_eq!(report.redaction_count, 0);
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("\"x\":10"));
    }

    #[test]
    fn malformed_line_passes_through() {
        let s = Scrubber::owasp_default();
        let ndjson = b"{not json}\n{\"text\":\"clean\"}\n";
        let (out, report) = s.scrub(ndjson).unwrap();
        assert_eq!(report.malformed_lines, 1);
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("{not json}"));
    }

    #[test]
    fn empty_input_emits_empty_output() {
        let s = Scrubber::owasp_default();
        let (out, report) = s.scrub(b"").unwrap();
        assert!(out.is_empty());
        assert_eq!(report.frame_count, 0);
    }

    #[test]
    fn empty_lines_skipped() {
        let s = Scrubber::owasp_default();
        let (_, report) = s.scrub(b"\n\n\n").unwrap();
        assert_eq!(report.frame_count, 0);
    }

    #[test]
    fn extra_pattern_applies() {
        let s = Scrubber::empty().try_with_extra(r"[A-Z]{3}-\d{4}").unwrap();
        let ndjson = br#"{"text":"order ABC-1234 confirmed"}"#;
        let (out, report) = s.scrub(ndjson).unwrap();
        assert!(String::from_utf8(out).unwrap().contains("[REDACTED]"));
        assert_eq!(report.redaction_count, 1);
    }

    #[test]
    fn invalid_pattern_errors() {
        let err = Scrubber::empty().try_with_extra("(").unwrap_err();
        assert!(matches!(err, ScrubberError::InvalidPattern { .. }));
    }

    #[test]
    fn ssn_pattern_matches() {
        let s = Scrubber::owasp_default();
        let ndjson = br#"{"text":"ssn 123-45-6789"}"#;
        let (out, report) = s.scrub(ndjson).unwrap();
        assert!(String::from_utf8(out).unwrap().contains("[REDACTED]"));
        assert!(report.redaction_count >= 1);
    }

    #[test]
    fn cc_pattern_matches_visa_format() {
        let s = Scrubber::owasp_default();
        let ndjson = br#"{"text":"card 4111 1111 1111 1111 last4"}"#;
        let (out, _) = s.scrub(ndjson).unwrap();
        assert!(String::from_utf8(out).unwrap().contains("[REDACTED]"));
    }

    #[test]
    fn multi_line_round_trip() {
        let s = Scrubber::owasp_default();
        let ndjson = b"{\"text\":\"a@b.co\"}\n{\"text\":\"clean\"}\n{\"text\":\"c@d.io\"}\n";
        let (out, report) = s.scrub(ndjson).unwrap();
        assert_eq!(report.frame_count, 3);
        assert_eq!(report.redaction_count, 2);
        let out = String::from_utf8(out).unwrap();
        assert_eq!(out.matches("[REDACTED]").count(), 2);
        assert_eq!(out.matches("clean").count(), 1);
    }
}
