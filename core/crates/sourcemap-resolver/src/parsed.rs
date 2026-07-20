//! Pure (no-I/O, no-cache) source map parsing + lookup.
//!
//! [`ParsedMap`] owns a single, fully-decoded Source Map V3
//! document. It exposes two primitives:
//!
//! - [`ParsedMap::resolve`] — `(line, column)` of a minified frame
//!   → [`Resolution`] of the original source position. Line numbers
//!   are 1-indexed and columns are 0-indexed on both sides, matching
//!   the convention every JS engine reports its stack frames in.
//! - [`ParsedMap::source_window`] — `±context` lines around a
//!   resolved position, pulled from `sourcesContent` if the bundler
//!   embedded it.
//!
//! The type is intentionally `Send + Sync` and cheap to share via
//! `Arc<ParsedMap>` (the underlying `sourcemap::SourceMap` is itself
//! immutable once parsed), which is what [`crate::ResolverCache`]
//! does internally.

use crate::error::{ParseError, ParseResult};
use sourcemap::{DecodedMap, SourceMap, decode_slice};

/// A parsed, immutable Source Map V3 document.
///
/// Construct via [`ParsedMap::parse`] from the raw bytes of a `.map`
/// file or an inline source-map comment payload. Once built, the
/// type is read-only and safe to share across threads — wrap it in
/// `Arc` to fan it out.
pub struct ParsedMap {
    inner: SourceMap,
}

impl ParsedMap {
    /// Parse a Source Map V3 document.
    ///
    /// Accepts both regular maps and `sections`-style index maps;
    /// the latter is flattened into a single regular map at parse
    /// time so the lookup path stays branch-free.
    ///
    /// # Errors
    ///
    /// - [`ParseError::Decode`] — bytes are not a Source Map V3 doc.
    /// - [`ParseError::Flatten`] — the doc is an index but a section
    ///   could not be flattened.
    /// - [`ParseError::UnsupportedFormat`] — the doc is a Hermes
    ///   (RN bytecode) map; use the dedicated Hermes stone instead.
    pub fn parse(bytes: &[u8]) -> ParseResult<Self> {
        let decoded = decode_slice(bytes).map_err(ParseError::Decode)?;
        let inner = match decoded {
            DecodedMap::Regular(sm) => sm,
            DecodedMap::Index(idx) => idx.flatten().map_err(ParseError::Flatten)?,
            DecodedMap::Hermes(_) => {
                return Err(ParseError::UnsupportedFormat { kind: "hermes" });
            }
        };
        Ok(Self { inner })
    }

    /// Resolve a minified `(line, column)` back to its original
    /// source position.
    ///
    /// - `line` is **1-indexed** (the convention every JS engine
    ///   uses when reporting stack frames). `0` is invalid and
    ///   returns `None` immediately without touching the parsed
    ///   map.
    /// - `column` is **0-indexed**.
    ///
    /// Returns `None` when the position is outside the map's
    /// coverage — the caller should leave the frame as-is in that
    /// case rather than synthesising a guess.
    ///
    /// "Outside coverage" means any of:
    ///
    /// - `line == 0` (invalid input — JS engines never report 0)
    /// - the requested `line` is past the last line the map has any
    ///   token on (the upstream `sourcemap` crate's `lookup_token`
    ///   falls back to the closest preceding token across line
    ///   boundaries, which silently synthesises a position from an
    ///   unrelated source — we reject that explicitly)
    /// - the map carries no token at-or-before `(line, column)` on
    ///   the requested line at all
    ///
    /// Within-line fuzzy lookup *is* legitimate — minified bundles
    /// span huge column ranges with a token roughly every statement,
    /// so a query at column 1280 mapping to the token at column 1024
    /// is the correct symbolication.
    #[must_use]
    pub fn resolve(&self, line: u32, column: u32) -> Option<Resolution> {
        if line == 0 {
            return None;
        }
        let dst_line = line - 1;
        let token = self.inner.lookup_token(dst_line, column)?;
        // Strict line match: reject the upstream's cross-line fallback
        // so we never invent a source position for a frame past EOF.
        if token.get_dst_line() != dst_line {
            return None;
        }
        Some(Resolution {
            file: token.get_source().map(str::to_owned),
            line: token.get_src_line().saturating_add(1),
            column: token.get_src_col(),
            function: token.get_name().map(str::to_owned),
            raw_line: line,
            raw_column: column,
            src_id: token.get_src_id(),
        })
    }

    /// `±context` lines from `sourcesContent` around the original
    /// source position pointed at by `src_id` and 0-indexed `line0`.
    ///
    /// Returns `None` if the bundler did not embed `sourcesContent`
    /// for the given source, the source id is unknown, or `line0`
    /// is past end-of-file. The window is silently clamped at the
    /// file boundaries — a request for `±5` near the top of a file
    /// will return fewer than five `before` lines without erroring.
    #[must_use]
    pub fn source_window(&self, src_id: u32, line0: usize, context: usize) -> Option<SourceWindow> {
        let view = self.inner.get_source_view(src_id)?;
        let rows: Vec<&str> = view.source().lines().collect();
        if line0 >= rows.len() {
            return None;
        }
        let start = line0.saturating_sub(context);
        let end = line0
            .checked_add(context)
            .and_then(|v| v.checked_add(1))
            .unwrap_or(rows.len())
            .min(rows.len());
        let before = rows[start..line0].iter().map(|s| (*s).to_owned()).collect();
        let at = rows[line0].to_owned();
        let after = rows[(line0 + 1)..end]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        Some(SourceWindow { before, at, after })
    }

    /// The number of distinct source files the map references.
    /// Mostly useful for observability and as a defensive sanity
    /// check on uploaded artefacts.
    #[must_use]
    pub fn source_count(&self) -> u32 {
        self.inner.get_source_count()
    }

    /// Whether the bundler embedded `sourcesContent` for every
    /// source the map references. When this is `false`,
    /// [`Self::source_window`] will return `None` for any source
    /// that lacks content.
    #[must_use]
    pub fn has_sources_content(&self) -> bool {
        (0..self.inner.get_source_count()).all(|i| self.inner.get_source_view(i).is_some())
    }
}

impl core::fmt::Debug for ParsedMap {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ParsedMap")
            .field("source_count", &self.source_count())
            .field("has_sources_content", &self.has_sources_content())
            .finish()
    }
}

/// One resolved frame — the output of [`ParsedMap::resolve`].
///
/// Fields are deliberately owned `String`s (not `&str` tied to the
/// map) so callers can stash the resolution into a typed frame
/// struct without lifetime ceremony. Frame structures in the
/// 钢筋 layer (`event-pipeline`'s `Frame`) consume `Resolution` by
/// value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    /// Original source filename, exactly as the map records it
    /// (typically a `webpack:///src/...` style URL). `None` if the
    /// map mapped the position to a token but the token had no
    /// associated source — pathological maps; treat as un-resolved.
    pub file: Option<String>,
    /// 1-indexed original source line.
    pub line: u32,
    /// 0-indexed original source column.
    pub column: u32,
    /// Original function name, if the map carries a `names[]` entry
    /// for the token. Many maps elide names for unnamed callables.
    pub function: Option<String>,
    /// The 1-indexed line the resolver was asked about — handy when
    /// the caller wants to keep both the raw minified position and
    /// the resolved one (the dashboard's "show source" path uses
    /// the raw line to reverse-look back through the same map).
    pub raw_line: u32,
    /// The 0-indexed column the resolver was asked about. Same
    /// rationale as [`Self::raw_line`].
    pub raw_column: u32,
    /// Internal source id (an index into the map's `sources[]`
    /// array). Stable for the lifetime of the [`ParsedMap`] and
    /// usable with [`ParsedMap::source_window`].
    pub src_id: u32,
}

/// A `±context` window around a resolved source line.
///
/// All three fields are 1-line-per-element. The `at` line is the
/// resolved position itself; `before` ends at the line above and
/// `after` starts at the line below.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceWindow {
    /// Up to `context` lines above the resolved line, in original
    /// file order (oldest first).
    pub before: Vec<String>,
    /// The resolved line itself.
    pub at: String,
    /// Up to `context` lines below the resolved line, in original
    /// file order.
    pub after: Vec<String>,
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
    use sourcemap::SourceMapBuilder;

    /// Build a deterministic two-source map fixture, suitable for
    /// unit-testing all four lookup paths (hit / miss / window
    /// hit / window miss).
    ///
    /// The map describes a minified bundle that concatenates two
    /// source files:
    ///
    ///   `src/foo.ts` (function name `hello`)
    ///   `src/bar.ts` (function name `world`)
    ///
    /// onto a single bundle line, at columns 0 and 64 respectively.
    fn fixture() -> Vec<u8> {
        let mut b = SourceMapBuilder::new(Some("bundle.js"));
        let foo_src = "function hello() {\n  return 42;\n}\n";
        let bar_src = "function world() {\n  return 7;\n}\n";
        let foo_id = b.add_source("src/foo.ts");
        let bar_id = b.add_source("src/bar.ts");
        b.set_source_contents(foo_id, Some(foo_src));
        b.set_source_contents(bar_id, Some(bar_src));
        let hello_name = b.add_name("hello");
        let world_name = b.add_name("world");
        // Bundle line 0 (1 from the JS engine's POV), columns 0 and 64.
        // Map them to src/foo.ts:1:9 and src/bar.ts:1:9 respectively
        // (the position of the function identifier in each source).
        b.add(0, 0, 0, 9, Some("src/foo.ts"), Some("hello"), false);
        b.add(0, 64, 0, 9, Some("src/bar.ts"), Some("world"), false);
        // Silence the unused-name warning — names are referenced via
        // `add()`'s `Some("…")` lookups so the IDs themselves are
        // not needed after registration.
        let _ = (foo_id, bar_id, hello_name, world_name);
        let mut out = Vec::new();
        b.into_sourcemap()
            .to_writer(&mut out)
            .expect("encode test fixture");
        out
    }

    #[test]
    fn parses_minimal_map() {
        let map = ParsedMap::parse(&fixture()).expect("parse fixture");
        assert_eq!(map.source_count(), 2);
        assert!(map.has_sources_content());
    }

    #[test]
    fn resolves_first_source() {
        let map = ParsedMap::parse(&fixture()).unwrap_or_else(|e| panic!("parse: {e}"));
        let r = map.resolve(1, 0).expect("lookup hits");
        assert_eq!(r.file.as_deref(), Some("src/foo.ts"));
        assert_eq!(r.line, 1);
        assert_eq!(r.column, 9);
        assert_eq!(r.function.as_deref(), Some("hello"));
        assert_eq!(r.raw_line, 1);
        assert_eq!(r.raw_column, 0);
    }

    #[test]
    fn resolves_second_source() {
        let map = ParsedMap::parse(&fixture()).unwrap_or_else(|e| panic!("parse: {e}"));
        let r = map.resolve(1, 64).expect("lookup hits");
        assert_eq!(r.file.as_deref(), Some("src/bar.ts"));
        assert_eq!(r.function.as_deref(), Some("world"));
    }

    #[test]
    fn line_zero_returns_none_without_lookup() {
        let map = ParsedMap::parse(&fixture()).unwrap_or_else(|e| panic!("parse: {e}"));
        assert!(map.resolve(0, 0).is_none());
        assert!(map.resolve(0, 1234).is_none());
    }

    #[test]
    fn out_of_coverage_returns_none() {
        let map = ParsedMap::parse(&fixture()).unwrap_or_else(|e| panic!("parse: {e}"));
        // Bundle has only line 1; line 999 is outside coverage.
        assert!(map.resolve(999, 0).is_none());
    }

    #[test]
    fn source_window_centered_returns_full_context() {
        let map = ParsedMap::parse(&fixture()).unwrap_or_else(|e| panic!("parse: {e}"));
        let r = map.resolve(1, 0).expect("first source");
        // foo.ts line 2 (0-indexed 1) → centred, before+after both 1.
        let w = map.source_window(r.src_id, 1, 1).expect("window");
        assert_eq!(w.before, vec!["function hello() {".to_owned()]);
        assert_eq!(w.at, "  return 42;");
        assert_eq!(w.after, vec!["}".to_owned()]);
    }

    #[test]
    fn source_window_at_file_start_clamps_before() {
        let map = ParsedMap::parse(&fixture()).unwrap_or_else(|e| panic!("parse: {e}"));
        let r = map.resolve(1, 0).expect("first source");
        let w = map.source_window(r.src_id, 0, 3).expect("window");
        assert!(w.before.is_empty());
        assert_eq!(w.at, "function hello() {");
        assert_eq!(w.after.len(), 2);
    }

    #[test]
    fn source_window_past_end_returns_none() {
        let map = ParsedMap::parse(&fixture()).unwrap_or_else(|e| panic!("parse: {e}"));
        assert!(map.source_window(0, 999, 1).is_none());
    }

    #[test]
    fn source_window_clamps_after_at_eof() {
        let map = ParsedMap::parse(&fixture()).unwrap_or_else(|e| panic!("parse: {e}"));
        // foo.ts has 3 lines; from line index 2, after should clamp to 0.
        let w = map.source_window(0, 2, 5).expect("window");
        assert_eq!(w.at, "}");
        assert!(w.after.is_empty());
    }

    #[test]
    fn source_window_huge_context_does_not_panic() {
        let map = ParsedMap::parse(&fixture()).unwrap_or_else(|e| panic!("parse: {e}"));
        // Pathological caller — `usize::MAX` must not overflow.
        let w = map
            .source_window(0, 1, usize::MAX)
            .expect("window with saturating clamp");
        assert!(!w.at.is_empty());
    }

    #[test]
    fn parse_rejects_garbage() {
        let err = ParsedMap::parse(b"not a sourcemap").expect_err("garbage");
        assert!(matches!(err, ParseError::Decode(_)));
    }

    #[test]
    fn parse_rejects_empty_input() {
        let err = ParsedMap::parse(&[]).expect_err("empty");
        assert!(matches!(err, ParseError::Decode(_)));
    }

    #[test]
    fn parse_rejects_truncated_json() {
        let err = ParsedMap::parse(b"{\"version\":3,\"sources\":").expect_err("truncated");
        assert!(matches!(err, ParseError::Decode(_)));
    }

    #[test]
    fn debug_impl_does_not_panic() {
        let map = ParsedMap::parse(&fixture()).unwrap_or_else(|e| panic!("parse: {e}"));
        let s = format!("{map:?}");
        assert!(s.contains("ParsedMap"));
        assert!(s.contains("source_count"));
    }
}
