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
fn reads_a_hermes_dialect_map() {
    // Every modern React Native build is Hermes, and
    // `react-native compose-source-maps` emits a Source Map V3
    // document carrying `x_facebook_sources` (and, for a real build,
    // `x_hermes_function_offsets`). The upstream decoder classifies
    // that as `DecodedMap::Hermes`, and this crate used to refuse it
    // outright — which meant an RN-first product could not read the
    // maps RN produces. insight's Android crashes sat unreadable with
    // a valid 21 MB composed map in the same release.
    //
    // A Hermes frame reports line 1 and a bytecode offset as its
    // column; the document's ordinary mappings are what resolve it.
    let doc = format!(
        r#"{{"version":3,"file":"index.android.bundle","sources":["src/a.ts"],"names":[],"mappings":"{}","sourcesContent":["const a = 1\nconst b = 2\n"],"x_facebook_sources":[[{{"names":["<global>"],"mappings":"AAA"}}]]}}"#,
        "AAAA"
    );
    let map = ParsedMap::parse(doc.as_bytes()).expect("a hermes map is a source map");
    let r = map
        .resolve(1, 0)
        .expect("its ordinary mappings still resolve");
    assert_eq!(r.file.as_deref(), Some("src/a.ts"));
    // and sourcesContent still reaches the reading window
    assert!(map.source_window(r.src_id, 0, 1).is_some());
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

#[test]
fn a_token_without_a_source_is_not_a_resolution() {
    // Hermes maps carry tokens for the engine's own
    // `InternalBytecode.js` frames with no source attached, and the
    // upstream reports their source line as `u32::MAX`. Returning a
    // Resolution for those put `InternalBytecode.js:4294967295` on
    // sixty-six production crashes.
    // A one-field segment maps a generated column and names no
    // source — legal Source Map V3, and what a Hermes engine frame
    // looks like.
    let doc = r#"{"version":3,"file":"b.js","sources":["src/a.ts"],"names":[],"mappings":"A"}"#;
    let map = ParsedMap::parse(doc.as_bytes()).expect("parse");
    assert!(map.resolve(1, 0).is_none());
}
