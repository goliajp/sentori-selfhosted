//! Pure (no-I/O, no-cache) ProGuard / R8 mapping parser + resolver.
//!
//! [`ParsedMapping`] owns the raw mapping bytes and an internal
//! `proguard::ProguardMapper` borrowed from them. The pairing is
//! held together by an `ouroboros`-generated self-referential
//! struct so callers can hand the parsed mapping around as a plain
//! value (no `Pin`, no lifetime parameter leaking into the public
//! API).
//!
//! ## What this primitive does
//!
//! - Accepts raw mapping bytes (UTF-8 text — what `mapping.txt`
//!   files on disk are).
//! - Validates them via the upstream `proguard` crate.
//! - Answers two lookup queries:
//!   - `resolve_class(obfuscated)` — class-only deobfuscation.
//!   - `resolve_method(class, method, line)` — full frame
//!     deobfuscation with R8 inline-call chain expansion.
//! - Surfaces the optional R8 `pg_map_id` UUID for callers that
//!   want to content-address mappings.
//!
//! ## What this primitive does NOT do
//!
//! - Network or filesystem I/O — the caller passes bytes.
//! - DB lookups (`server/src/symbolicate_android.rs`'s
//!   `load_mapping_by_debug_id` / `load_mapping_by_release` are
//!   the K-tier's job).
//! - Caching — [`crate::ResolverCache`] is composed on top.

use std::sync::Mutex;

use ouroboros::self_referencing;
use proguard::{ProguardMapper, ProguardMapping, StackFrame};
use uuid::Uuid;

use crate::error::{ParseError, ParseResult, ResolveError, ResolveResult};
use crate::frame::Frame;

/// Internal self-referential pair: owned bytes + `ProguardMapper`
/// borrowed from them, behind a `Mutex` so the public
/// [`ParsedMapping`] is `Send + Sync`. The upstream `ProguardMapper`
/// keeps internal `RefCell`s for lazy class-table indexing, mirroring
/// `addr2line::Context`'s shape; we use the same `Mutex`-around-
/// `ouroboros` pattern S7 settled on.
#[self_referencing]
struct MappingInner {
    bytes: Vec<u8>,
    #[borrows(bytes)]
    #[not_covariant]
    mapper: ProguardMapper<'this>,
}

/// A parsed, immutable ProGuard / R8 mapping.
///
/// Construct with [`ParsedMapping::parse`]; query with
/// [`ParsedMapping::resolve_class`] and
/// [`ParsedMapping::resolve_method`]. The type is `Send + Sync` and
/// meant to be shared via `Arc<ParsedMapping>` — internal
/// per-mapping caches lazily populate on first lookup, so
/// concurrent resolves serialise through a thin `Mutex` to preserve
/// the cache hit-rate across callers.
pub struct ParsedMapping {
    inner: Mutex<MappingInner>,
    summary: Summary,
}

/// Light metadata extracted at parse time so consumers can read it
/// without taking the resolver lock.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Summary {
    pg_map_id: Option<Uuid>,
    class_count: usize,
    method_count: usize,
    has_line_info: bool,
    byte_len: usize,
    compiler: Option<String>,
    compiler_version: Option<String>,
}

impl ParsedMapping {
    /// Parse a ProGuard / R8 mapping byte buffer.
    ///
    /// `bytes` is owned; the mapping stores it for the lifetime of
    /// the parsed mapper. Callers can drop their own reference
    /// after the call.
    ///
    /// # Errors
    ///
    /// - [`ParseError::InvalidUtf8`] — bytes are not valid UTF-8.
    /// - [`ParseError::InvalidMapping`] — bytes parse as UTF-8 but
    ///   the upstream's structural validation rejected them.
    pub fn parse(bytes: Vec<u8>) -> ParseResult<Self> {
        // Validate UTF-8 up-front so a corrupted upload returns a
        // clean error instead of a downstream panic from the
        // upstream parser if it ever tightens its expectations.
        std::str::from_utf8(&bytes)?;

        let summary = inspect_summary(&bytes)?;
        let inner = MappingInner::new(bytes, |b| ProguardMapper::new(ProguardMapping::new(b)));

        Ok(Self {
            inner: Mutex::new(inner),
            summary,
        })
    }

    /// Resolve an obfuscated class name to its original form.
    ///
    /// Returns `None` when the mapping has no entry for the
    /// requested class (the input was likely not obfuscated, or
    /// came from a library the mapping did not cover). The caller
    /// should keep the obfuscated form in that case rather than
    /// synthesise a guess.
    ///
    /// # Errors
    ///
    /// - [`ResolveError::Poisoned`] — internal lock poisoned by a
    ///   previous panicking caller; the mapping is unusable.
    pub fn resolve_class(&self, obfuscated: &str) -> ResolveResult<Option<String>> {
        let guard = self.inner.lock().map_err(|_| ResolveError::Poisoned)?;
        let resolved = guard.with_mapper(|m| m.remap_class(obfuscated).map(str::to_owned));
        drop(guard);
        Ok(resolved)
    }

    /// Resolve an obfuscated `(class, method, line)` to the
    /// original frame chain.
    ///
    /// The return value is **innermost-first**: index 0 is the
    /// deepest-inlined call, ancestors follow, the outermost
    /// having `is_inlined = false`. Empty `Vec` means no entry
    /// matched — keep the obfuscated frame as-is.
    ///
    /// `line` is the synthetic R8 line number reported by the
    /// crashing thread. Pass `0` if no line information was
    /// captured; the upstream resolver falls back to the unique
    /// top-level method (when one exists) in that case.
    ///
    /// # Errors
    ///
    /// - [`ResolveError::Poisoned`] — internal lock poisoned by
    ///   a previous panicking caller.
    pub fn resolve_method(
        &self,
        class: &str,
        method: &str,
        line: u32,
    ) -> ResolveResult<Vec<Frame>> {
        let guard = self.inner.lock().map_err(|_| ResolveError::Poisoned)?;
        let frames = guard.with_mapper(|m| {
            let line_us = usize::try_from(line).unwrap_or(0);
            let collected: Vec<StackFrame<'_>> = m
                .remap_frame(&StackFrame::new(class, method, line_us))
                .collect();
            let count = collected.len();
            collected
                .into_iter()
                .enumerate()
                .map(|(idx, sf)| Frame {
                    class: sf.class().to_owned(),
                    method: sf.method().to_owned(),
                    full_method: sf.full_method(),
                    file: sf.file().map(str::to_owned),
                    line: sf.line().and_then(|l| u32::try_from(l).ok()),
                    is_inlined: idx + 1 < count,
                })
                .collect()
        });
        drop(guard);
        Ok(frames)
    }

    /// The R8 `pg_map_id` UUID, if the mapping carried one.
    /// Stable identifier suitable for content-addressed cache
    /// keys — distinct uploads of the same mapping yield the same
    /// `pg_map_id`.
    #[must_use]
    pub const fn pg_map_id(&self) -> Option<Uuid> {
        self.summary.pg_map_id
    }

    /// Number of class entries in the mapping.
    #[must_use]
    pub const fn class_count(&self) -> usize {
        self.summary.class_count
    }

    /// Number of method entries (across all classes).
    #[must_use]
    pub const fn method_count(&self) -> usize {
        self.summary.method_count
    }

    /// `true` iff the mapping carries per-method line tables
    /// (R8's `1:5:void m():100:104 -> a` shape). Mappings emitted
    /// without `-keepattributes LineNumberTable` may carry only
    /// class / method names.
    #[must_use]
    pub const fn has_line_info(&self) -> bool {
        self.summary.has_line_info
    }

    /// Backing byte size — useful for observability (mapping files
    /// range from ~100 KB for small libraries to ~10 MB for full
    /// apps).
    #[must_use]
    pub const fn byte_len(&self) -> usize {
        self.summary.byte_len
    }

    /// The compiler that emitted the mapping, e.g. `"R8"` —
    /// extracted from the `# compiler: ...` header line.
    #[must_use]
    pub fn compiler(&self) -> Option<&str> {
        self.summary.compiler.as_deref()
    }

    /// The compiler version, e.g. `"8.2.42"` — extracted from the
    /// `# compiler_version: ...` header line.
    #[must_use]
    pub fn compiler_version(&self) -> Option<&str> {
        self.summary.compiler_version.as_deref()
    }
}

impl core::fmt::Debug for ParsedMapping {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ParsedMapping")
            .field("byte_len", &self.summary.byte_len)
            .field("class_count", &self.summary.class_count)
            .field("method_count", &self.summary.method_count)
            .field("has_line_info", &self.summary.has_line_info)
            .field("pg_map_id", &self.summary.pg_map_id)
            .finish_non_exhaustive()
    }
}

fn inspect_summary(bytes: &[u8]) -> ParseResult<Summary> {
    let mapping = ProguardMapping::new(bytes);
    if !mapping.is_valid() {
        return Err(ParseError::InvalidMapping);
    }
    let summary = mapping.summary();
    let pg_map_id = {
        let id = mapping.uuid();
        if id.is_nil() { None } else { Some(id) }
    };
    Ok(Summary {
        pg_map_id,
        class_count: summary.class_count(),
        method_count: summary.method_count(),
        has_line_info: mapping.has_line_info(),
        byte_len: bytes.len(),
        compiler: summary.compiler().map(str::to_owned),
        compiler_version: summary.compiler_version().map(str::to_owned),
    })
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
    use crate::test_fixtures::{MAPPING_WITH_INLINE, MAPPING_WITH_PG_MAP_ID, SIMPLE_MAPPING};

    #[test]
    fn parses_simple_mapping() {
        let m = ParsedMapping::parse(SIMPLE_MAPPING.as_bytes().to_vec()).expect("parse");
        assert!(m.class_count() >= 1);
        assert!(m.has_line_info());
    }

    #[test]
    fn rejects_non_utf8() {
        let err = ParsedMapping::parse(vec![0xff, 0xfe, 0xfd]).expect_err("bad utf8");
        assert!(matches!(err, ParseError::InvalidUtf8(_)));
    }

    #[test]
    fn rejects_non_mapping_text() {
        let err = ParsedMapping::parse(b"hello world\nnot a mapping".to_vec())
            .expect_err("not a mapping");
        assert!(matches!(err, ParseError::InvalidMapping));
    }

    #[test]
    fn resolves_class() {
        let m = ParsedMapping::parse(SIMPLE_MAPPING.as_bytes().to_vec()).expect("parse");
        let got = m.resolve_class("a.b.c").expect("ok");
        assert_eq!(
            got.as_deref(),
            Some("com.example.android.auth.LoginPresenter")
        );
    }

    #[test]
    fn resolves_unknown_class_to_none() {
        let m = ParsedMapping::parse(SIMPLE_MAPPING.as_bytes().to_vec()).expect("parse");
        let got = m.resolve_class("not.a.thing").expect("ok");
        assert!(got.is_none());
    }

    #[test]
    fn resolves_method_basic() {
        let m = ParsedMapping::parse(SIMPLE_MAPPING.as_bytes().to_vec()).expect("parse");
        // Synthetic range in SIMPLE_MAPPING is 42:42, so the runtime
        // line 42 maps back to original line 42.
        let frames = m.resolve_method("a.b.c", "a", 42).expect("ok");
        assert!(!frames.is_empty(), "got: {frames:?}");
        let leaf = &frames[0];
        assert_eq!(leaf.class, "com.example.android.auth.LoginPresenter");
        assert_eq!(leaf.method, "onLoginClick");
        assert_eq!(leaf.line, Some(42));
    }

    #[test]
    fn resolves_method_unknown_to_empty() {
        let m = ParsedMapping::parse(SIMPLE_MAPPING.as_bytes().to_vec()).expect("parse");
        let frames = m.resolve_method("not.a.class", "nope", 1).expect("ok");
        assert!(frames.is_empty());
    }

    #[test]
    fn resolves_inline_chain_innermost_first() {
        let m = ParsedMapping::parse(MAPPING_WITH_INLINE.as_bytes().to_vec()).expect("parse");
        // Synthetic range in MAPPING_WITH_INLINE is 1:5 with two
        // overlapping inline entries → calling line 3 yields the
        // full inlined chain.
        let frames = m.resolve_method("a.b.c", "a", 3).expect("ok");
        assert!(
            frames.len() >= 2,
            "expected ≥2 frames (inlined chain); got {frames:?}"
        );
        // Innermost (the inlined helper) at index 0; outermost at last.
        assert!(frames[0].is_inlined);
        assert!(!frames.last().expect("non-empty").is_inlined);
    }

    #[test]
    fn pg_map_id_is_some_when_header_present() {
        let m = ParsedMapping::parse(MAPPING_WITH_PG_MAP_ID.as_bytes().to_vec()).expect("parse");
        let id = m.pg_map_id().expect("pg_map_id present");
        // The synthesised UUID is content-addressed on the file's
        // bytes (the proguard crate hashes); we just assert it's
        // non-nil and stable across re-parses.
        assert!(!id.is_nil());
        let m2 =
            ParsedMapping::parse(MAPPING_WITH_PG_MAP_ID.as_bytes().to_vec()).expect("re-parse");
        assert_eq!(m2.pg_map_id(), Some(id));
    }

    #[test]
    fn byte_len_matches_input() {
        let raw = SIMPLE_MAPPING.as_bytes().to_vec();
        let len = raw.len();
        let m = ParsedMapping::parse(raw).expect("parse");
        assert_eq!(m.byte_len(), len);
    }

    #[test]
    fn debug_renders_counters() {
        let m = ParsedMapping::parse(SIMPLE_MAPPING.as_bytes().to_vec()).expect("parse");
        let s = format!("{m:?}");
        assert!(s.contains("ParsedMapping"));
        assert!(s.contains("byte_len"));
        assert!(s.contains("class_count"));
    }

    #[test]
    fn compiler_metadata_round_trips() {
        let m = ParsedMapping::parse(MAPPING_WITH_PG_MAP_ID.as_bytes().to_vec()).expect("parse");
        assert_eq!(m.compiler(), Some("R8"));
        assert_eq!(m.compiler_version(), Some("8.2.42"));
    }
}
