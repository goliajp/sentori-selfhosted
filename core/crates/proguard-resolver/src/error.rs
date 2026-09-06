//! Error types for the proguard-resolver stone.
//!
//! Two failure classes get distinct types so callers can branch
//! cleanly:
//!
//! 1. [`ParseError`] — turning raw mapping bytes into a
//!    [`crate::ParsedMapping`] failed (UTF-8 decode, structural
//!    validation, …).
//! 2. [`ResolveError`] — looking up an address against an already-
//!    parsed mapping failed (lock poisoned by a panicking caller).
//!
//! The crate's other failure modes — "no match found",
//! "obfuscated name was a no-op pass-through" — are represented as
//! `None` / empty `Vec`s in the resolve API rather than errors,
//! since they are normal control-flow outcomes the caller decides
//! how to handle.

use core::fmt;

/// Convenience alias for results returned by parse-side primitives.
pub type ParseResult<T> = Result<T, ParseError>;

/// Convenience alias for results returned by address resolution.
pub type ResolveResult<T> = Result<T, ResolveError>;

/// Errors returned when turning raw mapping bytes into a
/// [`crate::ParsedMapping`].
#[derive(Debug)]
#[non_exhaustive]
pub enum ParseError {
    /// Bytes were not valid UTF-8. ProGuard mapping files are
    /// always UTF-8 (Java source identifiers are UTF-8); any other
    /// encoding is corrupt input.
    InvalidUtf8(std::str::Utf8Error),

    /// Bytes parsed as text but the upstream `proguard` crate's
    /// validity check rejected them — typically a truncated file
    /// or a file that was uploaded as something other than a
    /// ProGuard mapping (a stack-trace text, a Java source file,
    /// …). Distinguishing "malformed mapping" from "non-mapping
    /// text" requires the upstream's structural validation which
    /// runs over the whole file, so we surface a single variant.
    InvalidMapping,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUtf8(_) => f.write_str("ProGuard mapping bytes are not valid UTF-8"),
            Self::InvalidMapping => f.write_str(
                "input failed ProGuard mapping validation (corrupt or non-mapping text)",
            ),
        }
    }
}

impl std::error::Error for ParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidUtf8(e) => Some(e),
            Self::InvalidMapping => None,
        }
    }
}

impl From<std::str::Utf8Error> for ParseError {
    fn from(e: std::str::Utf8Error) -> Self {
        Self::InvalidUtf8(e)
    }
}

/// Errors returned when resolving an obfuscated frame against an
/// already-parsed [`crate::ParsedMapping`].
///
/// "No match" is *not* an error here — `resolve_class` returns
/// `Option<String>` and `resolve_method` returns `Vec<Frame>`
/// (empty on miss). The only honest failure left is a poisoned
/// internal lock, which means a previous caller panicked while
/// resolving and the module is unusable until the surrounding
/// cache evicts it.
#[derive(Debug)]
#[non_exhaustive]
pub enum ResolveError {
    /// Another thread panicked while holding the mapping's
    /// internal lock. The caller should evict from any
    /// surrounding cache and re-parse if resolution is still
    /// needed.
    Poisoned,
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Poisoned => {
                f.write_str("ParsedMapping internal lock poisoned by a panicking thread")
            }
        }
    }
}

impl std::error::Error for ResolveError {}
