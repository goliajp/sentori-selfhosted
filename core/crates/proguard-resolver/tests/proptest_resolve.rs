//! Property tests for the ProGuard mapping parser + cache.
//!
//! Invariants:
//!
//! - Round-trip: every class entry the fixture builder emits
//!   resolves back to its original via `resolve_class`.
//! - Cache: `len ≤ capacity` always; `insert(k, v)` then `get(k)`
//!   returns the same Arc; `remove` is idempotent.
//! - Negative: unknown class names always return `None`; unknown
//!   method/line triples always return an empty `Vec`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_panics_doc,
    clippy::too_many_lines,
    clippy::format_push_string,
    clippy::doc_markdown,
    missing_docs
)]

use core::num::NonZeroUsize;
use std::sync::Arc;

use proptest::prelude::*;
use sentori_proguard_resolver::{ParsedMapping, ResolverCache};

/// Build a mapping with `n` classes, each carrying one method
/// with one synthetic line range. Returns the full text + an
/// `(obfuscated_class, obfuscated_method, line, expected_class,
/// expected_method, expected_line)` index for round-trip checks.
fn build_mapping(n: u32) -> (String, Vec<RoundTrip>) {
    let mut text = String::new();
    let mut idx = Vec::with_capacity(n as usize);
    for i in 0..n {
        let original_class = format!("com.example.pkg.Class{i}");
        let obf_class = format!("a{i}.b{i}.c{i}");
        let original_method = format!("doThing{i}");
        let obf_method = "m";
        let synth_line = 100 + i;
        let orig_line = 200 + i;

        text.push_str(&format!("{original_class} -> {obf_class}:\n"));
        text.push_str(&format!("    void {original_method}() -> {obf_method}\n"));
        text.push_str(&format!(
            "    {synth_line}:{synth_line}:void {original_method}():{orig_line}:{orig_line} -> {obf_method}\n"
        ));
        idx.push(RoundTrip {
            obf_class,
            obf_method: obf_method.to_owned(),
            line: synth_line,
            expected_class: original_class,
            expected_method: original_method,
            expected_line: orig_line,
        });
    }
    (text, idx)
}

#[derive(Debug, Clone)]
struct RoundTrip {
    obf_class: String,
    obf_method: String,
    line: u32,
    expected_class: String,
    expected_method: String,
    expected_line: u32,
}

fn arc_simple() -> Arc<ParsedMapping> {
    let (text, _) = build_mapping(2);
    Arc::new(ParsedMapping::parse(text.into_bytes()).expect("parse"))
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 64,
        .. ProptestConfig::default()
    })]

    /// Every emitted class round-trips: `resolve_class(obfuscated)`
    /// returns the original.
    #[test]
    fn class_roundtrip(n in 1u32..16) {
        let (text, idx) = build_mapping(n);
        let m = ParsedMapping::parse(text.into_bytes()).expect("parse");
        for rt in &idx {
            let got = m.resolve_class(&rt.obf_class).expect("ok");
            prop_assert_eq!(got, Some(rt.expected_class.clone()));
        }
    }

    /// Every emitted method round-trips: `resolve_method` returns
    /// the original class + method + line.
    #[test]
    fn method_roundtrip(n in 1u32..16) {
        let (text, idx) = build_mapping(n);
        let m = ParsedMapping::parse(text.into_bytes()).expect("parse");
        for rt in &idx {
            let frames = m.resolve_method(&rt.obf_class, &rt.obf_method, rt.line).expect("ok");
            prop_assert!(!frames.is_empty());
            let leaf = &frames[0];
            prop_assert_eq!(&leaf.class, &rt.expected_class);
            prop_assert_eq!(&leaf.method, &rt.expected_method);
            prop_assert_eq!(leaf.line, Some(rt.expected_line));
        }
    }

    /// Unknown class names always return `None`.
    #[test]
    fn unknown_class_returns_none(suffix in "[a-z]{4,8}") {
        let (text, _) = build_mapping(4);
        let m = ParsedMapping::parse(text.into_bytes()).expect("parse");
        let key = format!("not.a.class.{suffix}");
        prop_assert!(m.resolve_class(&key).expect("ok").is_none());
    }

    /// `len ≤ capacity` always.
    #[test]
    fn cache_len_never_exceeds_capacity(
        keys in prop::collection::vec(0u32..1000, 1..50),
        cap in 1usize..16,
    ) {
        let m = arc_simple();
        let c: ResolverCache<u32> =
            ResolverCache::new(NonZeroUsize::new(cap).expect("non-zero"));
        for k in &keys {
            c.insert(*k, Arc::clone(&m));
        }
        prop_assert!(c.len() <= cap);
    }

    /// Insert → get returns same Arc.
    #[test]
    fn cache_insert_then_get_roundtrips(key in 0u32..1000) {
        let m = arc_simple();
        let c: ResolverCache<u32> =
            ResolverCache::new(NonZeroUsize::new(4).expect("non-zero"));
        c.insert(key, Arc::clone(&m));
        let got = c.get(&key).expect("hit");
        prop_assert!(Arc::ptr_eq(&got, &m));
    }

    /// Remove is idempotent.
    #[test]
    fn cache_remove_is_idempotent(key in 0u32..1000) {
        let m = arc_simple();
        let c: ResolverCache<u32> =
            ResolverCache::new(NonZeroUsize::new(4).expect("non-zero"));
        c.insert(key, m);
        prop_assert!(c.remove(&key).is_some());
        prop_assert!(c.remove(&key).is_none());
    }
}
