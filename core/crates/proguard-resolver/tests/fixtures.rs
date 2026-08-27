//! Integration tests against hand-crafted ProGuard mappings.
//!
//! Drives the public API only (no internal modules) so any
//! re-export regression shows up here.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_panics_doc,
    clippy::doc_markdown,
    missing_docs
)]

use core::num::NonZeroUsize;
use std::sync::Arc;

use sentori_proguard_resolver::{ParseError, ParsedMapping, ResolverCache};

const FIXTURE: &str = "\
# compiler: R8
# compiler_version: 8.5.0
# pg_map_id: 0123456789abcdef
com.example.feature.SearchPresenter -> p.q.r:
    void onQuery() -> a
    100:100:void onQuery():100:100 -> a
    void formatResults() -> b
    200:210:void formatResults():200:210 -> b
";

#[test]
fn parses_via_public_api() {
    let m = ParsedMapping::parse(FIXTURE.as_bytes().to_vec()).expect("parse");
    assert!(m.class_count() >= 1);
    assert!(m.has_line_info());
    assert!(m.pg_map_id().is_some());
}

#[test]
fn resolves_class_via_public_api() {
    let m = ParsedMapping::parse(FIXTURE.as_bytes().to_vec()).expect("parse");
    assert_eq!(
        m.resolve_class("p.q.r").expect("ok").as_deref(),
        Some("com.example.feature.SearchPresenter")
    );
}

#[test]
fn resolves_method_via_public_api() {
    let m = ParsedMapping::parse(FIXTURE.as_bytes().to_vec()).expect("parse");
    let frames = m.resolve_method("p.q.r", "a", 100).expect("ok");
    assert_eq!(frames.first().map(|f| f.method.as_str()), Some("onQuery"));
    assert_eq!(frames.first().and_then(|f| f.line), Some(100));
}

#[test]
fn resolves_method_with_range_lookup() {
    let m = ParsedMapping::parse(FIXTURE.as_bytes().to_vec()).expect("parse");
    let frames = m.resolve_method("p.q.r", "b", 205).expect("ok");
    assert!(!frames.is_empty(), "expected match for line within range");
    assert_eq!(
        frames.first().map(|f| f.method.as_str()),
        Some("formatResults")
    );
}

#[test]
fn rejects_non_utf8_input() {
    let err = ParsedMapping::parse(vec![0xff, 0xff, 0xff]).expect_err("bad");
    assert!(matches!(err, ParseError::InvalidUtf8(_)));
}

#[test]
fn rejects_non_mapping_text() {
    let err =
        ParsedMapping::parse(b"just some prose, not a mapping at all".to_vec()).expect_err("bad");
    assert!(matches!(err, ParseError::InvalidMapping));
}

#[test]
fn cache_round_trip_via_public_api() {
    let cache: ResolverCache<String> = ResolverCache::new(NonZeroUsize::new(2).expect("non-zero"));
    let key = "release-1.0.0".to_owned();

    let mapping = cache
        .get_or_try_insert_with::<_, ParseError>(&key, || {
            ParsedMapping::parse(FIXTURE.as_bytes().to_vec()).map(Arc::new)
        })
        .expect("load");
    assert!(cache.get(&key).is_some());

    let frames = mapping.resolve_method("p.q.r", "a", 100).expect("ok");
    assert_eq!(frames.first().map(|f| f.method.as_str()), Some("onQuery"));
}
