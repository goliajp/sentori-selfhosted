//! Error types for the dwarf-resolver stone.
//!
//! Three orthogonal failure modes get distinct variants so callers
//! can branch on them (log, fall back, surface a dashboard hint)
//! without re-parsing the upstream error payload:
//!
//! 1. [`ParseError`] — failures producing a [`crate::DwarfModule`]
//!    from raw Mach-O bytes (object-format decode, DWARF section
//!    walk, addr2line context construction).
//! 2. [`SliceError`] — failures slicing a fat (universal) Mach-O
//!    into a single-arch payload.
//! 3. [`ResolveError`] — failures looking up a specific offset on
//!    an already-parsed module (gimli walk hit a malformed CU,
//!    inlined-frame iterator died, …).
//!
//! All three carry the upstream error in `source()` for tracing
//! pipelines that want the full chain; the top-level `Display`
//! prints a short, stable summary safe to surface to operators.

use core::fmt;

/// Convenience alias for results returned by parse-side primitives.
pub type ParseResult<T> = Result<T, ParseError>;

/// Convenience alias for results returned by fat-Mach-O slicing.
pub type SliceResult<T> = Result<T, SliceError>;

/// Convenience alias for results returned by address resolution.
pub type ResolveResult<T> = Result<T, ResolveError>;

/// Errors returned when turning raw Mach-O bytes into a
/// [`crate::DwarfModule`].
///
/// Variants are `#[non_exhaustive]` at both the enum and the inner
/// field level — additional context fields may be added later
/// without it being a breaking change.
#[derive(Debug)]
#[non_exhaustive]
pub enum ParseError {
    /// Bytes failed to decode as any object format the `object`
    /// crate understands (Mach-O, ELF, PE, WASM, COFF). Wraps the
    /// upstream's [`object::Error`].
    InvalidObject(object::Error),

    /// The bytes decoded as an object file but carry no DWARF
    /// debug info — typically a stripped binary. The caller has
    /// no way to symbolicate this module; it should keep the
    /// frame raw rather than synthesise a guess.
    NoDwarfSections,

    /// A DWARF section was present but malformed enough that
    /// `gimli` refused to parse it.
    InvalidDwarf(gimli::Error),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidObject(_) => f.write_str("object-format decode failed"),
            Self::NoDwarfSections => {
                f.write_str("object carries no DWARF debug info (stripped binary?)")
            }
            Self::InvalidDwarf(_) => f.write_str("DWARF section is malformed"),
        }
    }
}

impl std::error::Error for ParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidObject(e) => Some(e),
            Self::InvalidDwarf(e) => Some(e),
            Self::NoDwarfSections => None,
        }
    }
}

impl From<object::Error> for ParseError {
    fn from(e: object::Error) -> Self {
        Self::InvalidObject(e)
    }
}

impl From<gimli::Error> for ParseError {
    fn from(e: gimli::Error) -> Self {
        Self::InvalidDwarf(e)
    }
}

/// Errors returned by [`crate::MachoSlicer::slice`] when extracting
/// a single-arch payload from a fat (universal) Mach-O.
#[derive(Debug)]
#[non_exhaustive]
pub enum SliceError {
    /// Bytes failed to decode as Mach-O at all.
    InvalidObject(object::Error),

    /// Input is shorter than the fat header (8 bytes).
    TooShort,

    /// First four bytes match no known Mach-O magic (neither fat
    /// nor single-arch).
    UnrecognisedMagic([u8; 4]),

    /// Bytes are a single-arch Mach-O (not fat). The caller likely
    /// wanted to call `DwarfModule::from_bytes` directly instead.
    NotFat,

    /// The fat Mach-O contains no slice matching the requested
    /// arch. The error payload includes the arches actually
    /// present so the caller can surface a useful hint
    /// ("dSYM lacks an arm64 slice — re-run the upload with
    /// `lipo -archs` showing arm64").
    ArchNotFound {
        /// The arch the caller requested.
        requested: crate::Arch,
        /// The arches actually present in the fat header.
        available: Vec<crate::Arch>,
    },
}

impl fmt::Display for SliceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidObject(_) => f.write_str("Mach-O decode failed"),
            Self::TooShort => f.write_str("Mach-O input too short for any header"),
            Self::UnrecognisedMagic(m) => {
                write!(
                    f,
                    "unrecognised Mach-O magic 0x{:02x}{:02x}{:02x}{:02x}",
                    m[0], m[1], m[2], m[3]
                )
            }
            Self::NotFat => f.write_str("not a fat Mach-O (single-arch input)"),
            Self::ArchNotFound {
                requested,
                available,
            } => {
                write!(
                    f,
                    "arch {requested} not present in fat Mach-O (available: {available:?})"
                )
            }
        }
    }
}

impl std::error::Error for SliceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidObject(e) => Some(e),
            Self::TooShort
            | Self::UnrecognisedMagic(_)
            | Self::NotFat
            | Self::ArchNotFound { .. } => None,
        }
    }
}

impl From<object::Error> for SliceError {
    fn from(e: object::Error) -> Self {
        Self::InvalidObject(e)
    }
}

/// Errors returned when resolving an offset against an already-
/// parsed [`crate::DwarfModule`].
#[derive(Debug)]
#[non_exhaustive]
pub enum ResolveError {
    /// `gimli` returned an error walking the compilation unit or
    /// inlined-frame chain for the offset.
    Dwarf(gimli::Error),

    /// Another thread panicked while holding the module's internal
    /// lock. The module is unusable; the caller should evict it
    /// from any surrounding cache and re-parse the bytes if
    /// resolution is still needed.
    Poisoned,
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dwarf(_) => f.write_str("DWARF walk failed"),
            Self::Poisoned => {
                f.write_str("DwarfModule internal lock poisoned by a panicking thread")
            }
        }
    }
}

impl std::error::Error for ResolveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Dwarf(e) => Some(e),
            Self::Poisoned => None,
        }
    }
}

impl From<gimli::Error> for ResolveError {
    fn from(e: gimli::Error) -> Self {
        Self::Dwarf(e)
    }
}
