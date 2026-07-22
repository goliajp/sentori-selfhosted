//! Symbolicated Android stack frame — the output of
//! [`crate::ParsedMapping::resolve_method`].
//!
//! A single obfuscated `(class, method, line)` lookup against an
//! R8-emitted mapping can yield a *chain* of [`Frame`]s when the
//! line lands inside an inline-expanded call site — R8 records the
//! original call hierarchy so symbolicators can re-construct the
//! caller chain `inner ← middle ← outer`. We return the chain
//! innermost-first to match the convention `sentori-dwarf-resolver`
//! uses (the K-tier layer can render both side-by-side without
//! flipping iteration order).

/// One deobfuscated Android stack frame.
///
/// All fields are owned `String` / `Option<u32>` so callers can
/// stash the frame into typed structs on the 钢筋 side without
/// lifetime ceremony.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// Original (un-obfuscated) class name, e.g.
    /// `"com.example.android.auth.LoginPresenter"`.
    pub class: String,

    /// Original (un-obfuscated) method name, e.g. `"onLoginClick"`.
    /// Constructors keep the JVM convention name `"<init>"`.
    pub method: String,

    /// `class.method` joined — convenience for the common case
    /// where the dashboard surfaces a single string per frame.
    pub full_method: String,

    /// Original source filename, if the mapping carried one
    /// (`"LoginPresenter.kt"`). R8 strips real filenames by
    /// default; mappings emitted with `-keepattributes SourceFile`
    /// will preserve them.
    pub file: Option<String>,

    /// 1-indexed original source line, if the mapping carried one.
    /// `None` for frames where the mapping had only class /
    /// method-name info (R8 mappings without line tables, or
    /// when the synthetic line passed in is 0).
    pub line: Option<u32>,

    /// `true` iff this frame represents an inlined call rather
    /// than the lexically-innermost real method the synthetic
    /// line maps to. The deepest-inlined frame in the chain is
    /// at index 0 of [`crate::ParsedMapping::resolve_method`]'s
    /// return value; ancestors come after, the outermost having
    /// `is_inlined = false`.
    pub is_inlined: bool,
}
