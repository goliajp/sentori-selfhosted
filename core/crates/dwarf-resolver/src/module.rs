//! Pure (no-I/O, no-cache) DWARF address-resolution primitive.
//!
//! [`DwarfModule`] owns a single Mach-O byte buffer and the
//! [`addr2line::Context`] borrowed from it. The pairing is held
//! together by an `ouroboros`-generated self-referential struct so
//! callers can hand the parsed module around as a plain value (no
//! `Pin`, no lifetime parameter leaking into the public API) without
//! resorting to temp files like the legacy
//! `server/src/symbolicate_ios.rs` did.
//!
//! ## What this primitive does
//!
//! - Accepts raw Mach-O bytes (the contents of a dSYM bundle's
//!   inner Mach-O, or a non-stripped executable / `.o`).
//! - Parses the object, walks the DWARF sections, builds an
//!   `addr2line::Context`.
//! - Answers static-offset → frame-chain queries.
//!
//! ## What this primitive does NOT do
//!
//! - Network or filesystem I/O.
//! - Mach-O fat slicing (see [`crate::MachoSlicer`]).
//! - dSYM bundle directory unwrapping — the caller hands us the
//!   inner Mach-O, not the `.dSYM/` directory.
//! - Caching — the 钢筋 layer composes [`crate::ResolverCache`]
//!   on top.

use std::borrow::Cow;
use std::sync::Mutex;

use addr2line::Context;
use addr2line::FrameIter;
use addr2line::LookupResult;
use gimli::{EndianSlice, RunTimeEndian, Section, SectionId};
use object::{Object, ObjectSection};
use ouroboros::self_referencing;

use crate::error::{ParseError, ParseResult, ResolveError, ResolveResult};
use crate::frame::Frame;

/// Internal self-referential pair: owned bytes + `addr2line::Context`
/// borrowed from them. Hidden behind a `Mutex` in the public
/// [`DwarfModule`] type so callers can share `Arc<DwarfModule>`
/// across threads — `addr2line::Context` is `Send` but not `Sync`
/// (it lazily populates internal caches on first lookup via
/// `RefCell`), and the `Mutex` is the smallest sufficient guard.
#[self_referencing]
struct ModuleInner {
    bytes: Vec<u8>,
    #[borrows(bytes)]
    #[not_covariant]
    context: Context<EndianSlice<'this, RunTimeEndian>>,
}

/// A parsed, immutable DWARF-bearing Mach-O module.
///
/// Construct with [`DwarfModule::from_bytes`]; query with
/// [`DwarfModule::resolve`]. The type is `Send + Sync` and meant
/// to be shared via `Arc<DwarfModule>` — the internal
/// `addr2line::Context` lazily fills its own per-CU caches on
/// first lookup, so concurrent resolves serialise through a thin
/// `Mutex` to preserve the cache hit-rate across callers (rather
/// than each thread re-parsing the same CU).
pub struct DwarfModule {
    inner: Mutex<ModuleInner>,
    byte_len: usize,
}

impl DwarfModule {
    /// Parse a Mach-O (or other `object`-supported format) byte
    /// buffer into a ready-to-query DWARF module.
    ///
    /// `bytes` is owned (the module stores it for the lifetime of
    /// the parsed context) so the caller can drop their reference
    /// after the call. To avoid copying a large buffer, pass an
    /// owned `Vec<u8>` you no longer need; the module never mutates
    /// it.
    ///
    /// # Errors
    ///
    /// - [`ParseError::InvalidObject`] — bytes are not a recognised
    ///   object format.
    /// - [`ParseError::NoDwarfSections`] — the object is well-formed
    ///   but carries no DWARF (stripped binary; the caller should
    ///   surface a "dSYM missing" hint).
    /// - [`ParseError::InvalidDwarf`] — DWARF is present but
    ///   malformed; `gimli` refused to parse it.
    pub fn from_bytes(bytes: Vec<u8>) -> ParseResult<Self> {
        let byte_len = bytes.len();
        let inner = ModuleInner::try_new(bytes, |b| build_context(b.as_slice()))?;
        Ok(Self {
            inner: Mutex::new(inner),
            byte_len,
        })
    }

    /// Resolve a static offset (program counter minus image base —
    /// i.e. with the ASLR slide already subtracted) to the inlined
    /// frame chain at that address.
    ///
    /// The returned `Vec` is ordered **innermost-first**: index 0
    /// is the deepest-inlined call, and ancestors (caller chain)
    /// follow. The last frame's `is_inlined` is `false` — it is the
    /// lexically-innermost real function the PC lives in.
    ///
    /// Returns an empty `Vec` if the offset is outside any
    /// compilation unit's covered ranges (the caller should treat
    /// the frame as un-symbolicated and keep the raw PC).
    ///
    /// # Errors
    ///
    /// - [`ResolveError::Dwarf`] — `gimli` returned an error
    ///   walking the inlined-frame chain (corrupt CU or abbrev
    ///   table). The caller may want to log and skip this single
    ///   frame rather than abort the whole stack.
    /// - [`ResolveError::Poisoned`] — another thread panicked while
    ///   holding the internal lock. The module is unusable; the
    ///   caller should evict it from any surrounding cache and
    ///   re-parse the bytes if needed.
    pub fn resolve(&self, offset: u64) -> ResolveResult<Vec<Frame>> {
        let guard = self.inner.lock().map_err(|_| ResolveError::Poisoned)?;
        guard.with_context(|ctx| resolve_with(ctx, offset))
    }

    /// The size (in bytes) of the backing Mach-O buffer. Useful
    /// for observability — a typical iOS arm64 dSYM ranges 5-50 MB.
    #[must_use]
    pub const fn byte_len(&self) -> usize {
        self.byte_len
    }
}

impl core::fmt::Debug for DwarfModule {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DwarfModule")
            .field("byte_len", &self.byte_len)
            .finish_non_exhaustive()
    }
}

/// Parse `bytes` as an object file, walk its DWARF sections, and
/// build the [`addr2line::Context`] in one shot. Used inside the
/// `ouroboros` constructor closure so the lifetimes line up — the
/// returned `Context` borrows from `bytes` (the section data the
/// `object` crate's section accessors return is borrowed from the
/// input slice, *not* from the `object::File` itself, so dropping
/// `File` at the end of this function leaves the `Context` valid).
fn build_context(bytes: &[u8]) -> ParseResult<Context<EndianSlice<'_, RunTimeEndian>>> {
    let object_file = object::File::parse(bytes)?;
    let endian = if object_file.is_little_endian() {
        RunTimeEndian::Little
    } else {
        RunTimeEndian::Big
    };

    let load = |id: SectionId| -> Result<EndianSlice<'_, RunTimeEndian>, gimli::Error> {
        Ok(EndianSlice::new(load_section(&object_file, id), endian))
    };

    let dwarf = gimli::Dwarf::load(load)?;
    if !has_any_debug_info(&dwarf) {
        return Err(ParseError::NoDwarfSections);
    }
    Ok(Context::from_dwarf(dwarf)?)
}

/// Return the raw bytes of `section_id` from `object_file`, or an
/// empty slice if the section is absent. Empty-on-absent matches
/// what `gimli::Dwarf::load` expects: missing sections are valid
/// (they just mean the corresponding DWARF feature is unused).
///
/// `section.uncompressed_data()` may return `Cow::Owned` for
/// compressed sections (CMU's `__zdebug_*` convention), in which
/// case we would lose the lifetime needed for the self-referential
/// `Context`. We avoid the issue by using the raw `data()` accessor
/// and pessimistically refusing compressed DWARF — modern Mach-O
/// dSYMs do not compress DWARF, so this is a non-issue in practice
/// (the legacy `symbolicate_ios.rs` had the same constraint).
fn load_section<'a>(object_file: &object::File<'a>, id: SectionId) -> &'a [u8] {
    // Try the canonical `.debug_*` name first (works for ELF). If
    // absent (Mach-O renames everything to `__debug_*`), fall back
    // to the segment-prefixed form.
    let primary = object_file
        .section_by_name(id.name())
        .map_or(&[][..], |s| s.data().unwrap_or(&[]));
    if !primary.is_empty() {
        return primary;
    }
    let mach_name = format!("__{}", id.name().trim_start_matches('.'));
    object_file
        .section_by_name(&mach_name)
        .map_or(&[][..], |s| s.data().unwrap_or(&[]))
}

/// `true` iff at least one of the DWARF sections that actually
/// carry symbolication info (debug_info + debug_line) is non-empty.
/// Used to distinguish "stripped object" from "object with empty
/// abbrev table but real line info" — the latter is fine, the
/// former we reject up-front so the caller gets a clear error.
fn has_any_debug_info<R: gimli::Reader>(dwarf: &gimli::Dwarf<R>) -> bool {
    !dwarf.debug_info.reader().is_empty() || !dwarf.debug_line.reader().is_empty()
}

/// Drive `addr2line::Context::find_frames` to completion and
/// collect the inlined chain into a `Vec<Frame>`. The
/// `LookupResult` API supports DWARF 5's split-DWARF supplementary
/// files (where resolving may require a second pass against a
/// supplementary file) — we don't support split DWARF in v0.1, so
/// we treat any continuation request as "no info".
fn resolve_with<R: gimli::Reader>(ctx: &Context<R>, offset: u64) -> ResolveResult<Vec<Frame>> {
    let iter = match ctx.find_frames(offset) {
        LookupResult::Output(result) => result?,
        LookupResult::Load { .. } => return Ok(Vec::new()),
    };
    collect_frames(iter)
}

fn collect_frames<R: gimli::Reader>(mut iter: FrameIter<R>) -> ResolveResult<Vec<Frame>> {
    let mut out: Vec<Frame> = Vec::new();
    while let Some(frame) = iter.next()? {
        let function = frame.function.map(|fname| {
            fname
                .demangle()
                .map_or_else(|_| decode_lossy(&fname.name), Cow::into_owned)
        });
        let (file, line, column) = frame.location.map_or((None, None, None), |loc| {
            (loc.file.map(str::to_owned), loc.line, loc.column)
        });
        out.push(Frame {
            function,
            file,
            line,
            column,
            is_inlined: true,
        });
    }
    // The last frame the iterator emits is the outermost non-
    // inlined function; flip its flag.
    if let Some(last) = out.last_mut() {
        last.is_inlined = false;
    }
    Ok(out)
}

/// Best-effort name decode for symbols `addr2line`'s demangler
/// could not handle (Swift-mangled names, non-UTF-8 raw bytes,
/// etc.). We keep whatever the symbol table held verbatim — the
/// dashboard renders the raw form and operators can copy it into
/// `swift demangle` themselves.
fn decode_lossy<R: gimli::Reader>(name: &R) -> String {
    let bytes = name.to_slice().unwrap_or(Cow::Borrowed(&[]));
    String::from_utf8_lossy(&bytes).into_owned()
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
    use crate::test_fixtures::synthetic_macho_with_dwarf;

    #[test]
    fn rejects_garbage_bytes() {
        let err = DwarfModule::from_bytes(vec![1, 2, 3, 4]).expect_err("garbage");
        assert!(matches!(err, ParseError::InvalidObject(_)));
    }

    #[test]
    fn rejects_empty_bytes() {
        let err = DwarfModule::from_bytes(Vec::new()).expect_err("empty");
        assert!(matches!(err, ParseError::InvalidObject(_)));
    }

    #[test]
    fn parses_synthetic_macho() {
        let fx = synthetic_macho_with_dwarf();
        let module = DwarfModule::from_bytes(fx.bytes.clone()).expect("parse");
        assert_eq!(module.byte_len(), fx.bytes.len());
    }

    #[test]
    fn resolves_known_address() {
        let fx = synthetic_macho_with_dwarf();
        let module = DwarfModule::from_bytes(fx.bytes.clone()).expect("parse");
        let frames = module.resolve(fx.known_offset).expect("resolve");
        assert!(!frames.is_empty(), "expected at least one frame");
        let leaf = &frames[0];
        assert_eq!(leaf.function.as_deref(), Some(fx.known_function));
        // addr2line joins the comp_dir + decl_file when resolving, so
        // the surfaced path is the joined form. We assert that the
        // resolved file path ends with `fx.known_file` rather than
        // pinning the exact prefix — different DWARF emitters may
        // store comp_dir differently.
        let file = leaf.file.as_deref().expect("file");
        assert!(
            file.ends_with(fx.known_file),
            "file = {file:?}, want suffix {:?}",
            fx.known_file
        );
        assert_eq!(leaf.line, Some(fx.known_line));
        assert!(!frames.last().expect("last frame").is_inlined);
    }

    #[test]
    fn resolves_unknown_address_to_empty_vec() {
        let fx = synthetic_macho_with_dwarf();
        let module = DwarfModule::from_bytes(fx.bytes).expect("parse");
        let frames = module.resolve(0xDEAD_BEEF_DEAD_BEEF).expect("resolve");
        assert!(frames.is_empty());
    }

    #[test]
    fn debug_renders_byte_len() {
        let fx = synthetic_macho_with_dwarf();
        let module = DwarfModule::from_bytes(fx.bytes).expect("parse");
        let s = format!("{module:?}");
        assert!(s.contains("DwarfModule"));
        assert!(s.contains("byte_len"));
    }
}
