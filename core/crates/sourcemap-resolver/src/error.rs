//! Error types for the sourcemap-resolver stone.
//!
//! The crate distinguishes three classes of failure so callers can
//! log diagnostics (or surface dashboard hints) without re-parsing
//! the underlying upstream error:
//!
//! 1. [`ParseError::Decode`] — the bytes are not a Source Map V3
//!    document at all (truncated, non-UTF-8 JSON, wrong schema).
//! 2. [`ParseError::Flatten`] — the bytes parsed as a
//!    `SourceMapIndex` (a split-files map referencing sections via
//!    URLs / inline sections) but flattening into a single regular
//!    map failed (typically an inner section that was itself
//!    malformed).
//! 3. [`ParseError::UnsupportedFormat`] — the bytes parsed as a
//!    Hermes (React Native bytecode) map, which uses a different
//!    lookup primitive. Hermes resolution lives in a dedicated
//!    stone alongside `dwarf-resolver`; this crate refuses to
//!    pretend.

use core::fmt;

/// Convenience `Result` alias used across the crate's parse paths.
pub type ParseResult<T> = Result<T, ParseError>;

/// Errors returned when parsing raw bytes into a
/// [`crate::ParsedMap`].
///
/// All variants are non-exhaustive at the variant level — additional
/// fields may be added later without it being a breaking change.
#[derive(Debug)]
#[non_exhaustive]
pub enum ParseError {
    /// The input is not a valid Source Map V3 document. Wraps the
    /// upstream parser's error verbatim — useful for tracing /
    /// telemetry but treat the payload as opaque.
    Decode(sourcemap::Error),

    /// The input parsed as a `SourceMapIndex` (split-files map) but
    /// flattening sections into a single regular map failed.
    Flatten(sourcemap::Error),

    /// The input parsed as a recognised but unsupported sourcemap
    /// dialect (e.g. Hermes bytecode maps). Use a dialect-specific
    /// stone instead — this crate is JS-only.
    UnsupportedFormat {
        /// Human-readable name of the unsupported dialect, e.g.
        /// `"hermes"`. Stable string — safe to match in tests.
        kind: &'static str,
    },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(_) => f.write_str("source map decode failed"),
            Self::Flatten(_) => f.write_str("source map index flatten failed"),
            Self::UnsupportedFormat { kind } => write!(
                f,
                "unsupported source map dialect ({kind}) — use the dedicated resolver"
            ),
        }
    }
}

impl std::error::Error for ParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Decode(e) | Self::Flatten(e) => Some(e),
            Self::UnsupportedFormat { .. } => None,
        }
    }
}
