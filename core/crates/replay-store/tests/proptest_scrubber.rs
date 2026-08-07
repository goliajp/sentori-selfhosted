//! Property tests for [`Scrubber`] invariants.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::items_after_statements
)]

use proptest::prelude::*;
use sentori_replay_store::{REDACTED_PLACEHOLDER, Scrubber};

proptest! {
    #[test]
    fn email_is_always_redacted(local in "[a-z]{1,10}", domain in "[a-z]{1,8}", tld in "[a-z]{2,4}") {
        let s = Scrubber::owasp_default();
        let email = format!("{local}@{domain}.{tld}");
        let ndjson = format!(r#"{{"text":"contact {email}"}}"#);
        let (out, report) = s.scrub(ndjson.as_bytes()).expect("scrub");
        let out_str = String::from_utf8(out).expect("utf8");
        prop_assert!(out_str.contains(REDACTED_PLACEHOLDER));
        prop_assert!(!out_str.contains(&email), "leaked email: {email} in {out_str}");
        prop_assert!(report.redaction_count >= 1);
    }

    #[test]
    fn clean_text_passes_through(text in "[a-z ]{1,128}") {
        // Avoid the rare case where random ASCII letters happen
        // to look like an email/phone/CC/SSN.
        prop_assume!(!text.contains('@'));
        prop_assume!(!text.chars().any(|c| c.is_ascii_digit()));
        let s = Scrubber::owasp_default();
        let ndjson = format!(r#"{{"text":"{text}"}}"#);
        let (out, report) = s.scrub(ndjson.as_bytes()).expect("scrub");
        let out_str = String::from_utf8(out).expect("utf8");
        prop_assert!(out_str.contains(&text), "lost clean text: {text}");
        prop_assert_eq!(report.redaction_count, 0);
    }

    #[test]
    fn line_count_preserved(lines in proptest::collection::vec("[a-z ]{1,32}", 1..10)) {
        // Skip lines that happen to look like phones / emails / etc.
        for l in &lines {
            prop_assume!(!l.contains('@'));
            prop_assume!(!l.chars().any(|c| c.is_ascii_digit()));
        }
        let s = Scrubber::owasp_default();
        use std::fmt::Write as _;
        let mut ndjson = String::new();
        for t in &lines {
            writeln!(&mut ndjson, "{{\"text\":\"{t}\"}}").unwrap();
        }
        let (out, report) = s.scrub(ndjson.as_bytes()).expect("scrub");
        let out_str = String::from_utf8(out).expect("utf8");
        let actual_lines = out_str.lines().count();
        prop_assert_eq!(actual_lines, lines.len());
        prop_assert_eq!(report.frame_count, i32::try_from(lines.len()).unwrap());
    }

    #[test]
    fn scrub_is_idempotent(text in "[a-z ]{1,32}") {
        prop_assume!(!text.contains('@'));
        prop_assume!(!text.chars().any(|c| c.is_ascii_digit()));
        let s = Scrubber::owasp_default();
        let ndjson = format!(r#"{{"text":"foo {text} alice@x.io"}}"#);
        let (first, _) = s.scrub(ndjson.as_bytes()).expect("scrub-1");
        let (second, _) = s.scrub(&first).expect("scrub-2");
        prop_assert_eq!(first, second, "scrubbing scrubbed bytes must be a no-op");
    }
}

#[test]
fn empty_pack_redacts_nothing() {
    let s = Scrubber::empty();
    let ndjson = br#"{"text":"alice@example.com"}"#;
    let (out, report) = s.scrub(ndjson).unwrap();
    assert_eq!(report.redaction_count, 0);
    let s = String::from_utf8(out).unwrap();
    assert!(s.contains("alice@example.com"));
}
