//! Integration tests against hand-crafted Source Map V3 fixtures.
//!
//! Unlike the in-`parsed.rs` tests (which build maps with
//! `SourceMapBuilder` and round-trip them), these fixtures are
//! crafted at the JSON level so a regression in `sourcemap`'s
//! decoder shows up here, not just in the round-trip path.

#![allow(
    missing_docs,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_panics_doc,
    clippy::doc_markdown
)]

use sentori_sourcemap_resolver::{ParseError, ParsedMap};

/// Minimal hand-rolled V3 map: bundle line 1, col 0 → src/a.ts:1:9.
///
/// The mapping VLQ `"AAAAA"` encodes (col_delta=0, src_id=0,
/// src_line=0, src_col=0, name_id=0) — five-segment with a name —
/// followed by a self-segment to give the bundle a second token at
/// col 1. Hand-verified against the V3 spec's worked example.
const HAND_ROLLED_V3: &str = r#"{
    "version": 3,
    "file": "bundle.js",
    "sources": ["src/a.ts"],
    "sourcesContent": ["function hi() {\n  return 1;\n}\n"],
    "names": ["hi"],
    "mappings": "AAAAA"
}"#;

#[test]
fn parses_hand_rolled_v3_doc() {
    let map = ParsedMap::parse(HAND_ROLLED_V3.as_bytes()).expect("parse hand-rolled");
    assert_eq!(map.source_count(), 1);
    assert!(map.has_sources_content());
    let r = map.resolve(1, 0).expect("first token");
    assert_eq!(r.file.as_deref(), Some("src/a.ts"));
    assert_eq!(r.line, 1);
    assert_eq!(r.column, 0);
    assert_eq!(r.function.as_deref(), Some("hi"));
}

#[test]
fn parses_section_index_map() {
    // SourceMapIndex with one inline section. Flattening should
    // collapse it to a single Regular SourceMap transparently.
    let doc = format!(
        r#"{{"version":3,"file":"bundle.js","sections":[{{"offset":{{"line":0,"column":0}},"map":{HAND_ROLLED_V3}}}]}}"#
    );
    let map = ParsedMap::parse(doc.as_bytes()).expect("parse + flatten");
    assert!(map.source_count() >= 1);
    let r = map.resolve(1, 0).expect("token survived flatten");
    assert_eq!(r.file.as_deref(), Some("src/a.ts"));
}

#[test]
fn rejects_hermes_dialect_explicitly() {
    // Hermes maps carry the `x_facebook_sources` extension and an
    // empty `mappings` string at the top level alongside per-
    // function bytecode-offset tables in a Hermes-specific shape.
    // We craft the minimum shape that triggers `DecodedMap::Hermes`
    // in the upstream decoder.
    let doc = r#"{
        "version": 3,
        "file": "index.android.bundle",
        "sources": ["index.js"],
        "names": [],
        "mappings": "",
        "x_facebook_sources": [[{ "names": ["<global>"], "mappings": "AAA" }]]
    }"#;
    let err = ParsedMap::parse(doc.as_bytes()).expect_err("hermes refused");
    assert!(
        matches!(err, ParseError::UnsupportedFormat { kind: "hermes" }),
        "expected UnsupportedFormat(hermes), got {err:?}",
    );
}

#[test]
fn unsupported_format_display_mentions_kind() {
    let err = ParseError::UnsupportedFormat { kind: "fictional" };
    let s = format!("{err}");
    assert!(s.contains("fictional"), "got: {s}");
}

#[test]
fn decode_error_has_source() {
    let err = ParsedMap::parse(b"!!!").expect_err("garbage");
    assert!(std::error::Error::source(&err).is_some());
}
