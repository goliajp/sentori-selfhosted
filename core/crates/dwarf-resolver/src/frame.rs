//! Symbolicated stack frame — the output of
//! [`crate::DwarfModule::resolve`].
//!
//! Each call to `resolve` returns a *chain* of [`Frame`]s, not a
//! single one. DWARF preserves the inlined-call hierarchy: a single
//! instruction pointer may correspond to "f calling g inlined into
//! h inlined into …" — `addr2line` walks the chain leaf-to-root,
//! and we hand it back to the caller in the same order so the
//! dashboard can render `g (inlined) ← h (inlined) ← f` without
//! re-discovering the inlining edges.

/// A single symbolicated stack frame.
///
/// Fields are owned `String`s and `Option`s so callers can hand
/// the frame straight into a typed `Frame` struct on the
/// 钢筋 side without lifetime ceremony. `None` for any field
/// means "DWARF carried no info for this dimension at this
/// program-counter", not "the resolver failed" — see
/// [`crate::ResolveError`] for the latter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// Symbol name, demangled when possible. For C++ / Rust the
    /// demangled form ("`std::vector::push_back`") is preferred;
    /// for C the mangled and demangled forms are identical.
    pub function: Option<String>,

    /// Source filename, exactly as the compiler recorded it
    /// (typically an absolute path on the build host). The 钢筋
    /// layer is responsible for rewriting it to a project-relative
    /// form if the dashboard prefers that.
    pub file: Option<String>,

    /// 1-indexed source line.
    pub line: Option<u32>,

    /// 1-indexed source column. Often `None` — many compilers omit
    /// column info to save dSYM size.
    pub column: Option<u32>,

    /// `true` iff this frame represents an inlined call rather than
    /// the lexically-innermost function the instruction pointer
    /// actually lives in. The innermost (deepest-inlined) frame in
    /// the chain is at index 0 of [`crate::DwarfModule::resolve`]'s
    /// return value; ancestors come after.
    pub is_inlined: bool,
}

impl Frame {
    /// Construct a frame with all fields unset. Useful for tests
    /// that want to anchor on the `is_inlined` flag alone.
    #[must_use]
    pub const fn empty(is_inlined: bool) -> Self {
        Self {
            function: None,
            file: None,
            line: None,
            column: None,
            is_inlined,
        }
    }
}
