//! Error types for the geoip-reader stone.

use core::fmt;

/// Convenience alias for parse-side primitives.
pub type ParseResult<T> = Result<T, ParseError>;

/// Errors returned by [`crate::MmdbReader::from_bytes`].
#[derive(Debug)]
#[non_exhaustive]
pub enum ParseError {
    /// Bytes are not a valid MaxMind .mmdb document. The
    /// underlying parser ran into a structural / metadata
    /// problem (truncated file, unknown format version, …).
    InvalidDatabase(maxminddb::MaxMindDbError),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDatabase(_) => f.write_str("input is not a valid MaxMind .mmdb document"),
        }
    }
}

impl std::error::Error for ParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidDatabase(e) => Some(e),
        }
    }
}

impl From<maxminddb::MaxMindDbError> for ParseError {
    fn from(e: maxminddb::MaxMindDbError) -> Self {
        Self::InvalidDatabase(e)
    }
}
