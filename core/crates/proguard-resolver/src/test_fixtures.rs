//! Inline ProGuard mapping fixtures for the crate's tests.
//!
//! ProGuard mapping files are plain UTF-8 text, so there is no
//! binary-fixture problem like S7 had. We keep these as `&'static
//! str` constants so any test can grab one without a builder
//! closure.
//!
//! Coverage:
//!
//! - [`SIMPLE_MAPPING`] — one class with one method and one line
//!   table entry. Smallest input that exercises class + method +
//!   line resolution end-to-end.
//! - [`MAPPING_WITH_INLINE`] — same class, but the obfuscated
//!   `a` method's range covers two original-line ranges (R8's
//!   inline-expansion grammar). Exercises the
//!   `resolve_method → Vec<Frame>` chain path.
//! - [`MAPPING_WITH_PG_MAP_ID`] — same as `SIMPLE_MAPPING` plus
//!   the R8 `# pg_map_id` and `# compiler` headers, exercising
//!   the metadata extraction path.
//! - [`LARGE_MAPPING`] — synthetic 200-class fixture for bench
//!   parse-cost measurement.

#![cfg(test)]
#![allow(missing_docs, clippy::format_push_string)]

pub(crate) const SIMPLE_MAPPING: &str = "\
com.example.android.auth.LoginPresenter -> a.b.c:
    void onLoginClick() -> a
    42:42:void onLoginClick():42:42 -> a
";

/// One class, one obfuscated method, R8 inline expansion: the
/// obfuscated `a()` covers two real frames (helper inlined into
/// caller).
pub(crate) const MAPPING_WITH_INLINE: &str = "\
com.example.android.auth.LoginPresenter -> a.b.c:
    void onLoginClick() -> a
    void doAuth() -> a
    1:5:void doAuth():100:104 -> a
    1:5:void onLoginClick(int):50:54 -> a
";

/// Mapping with R8 metadata headers (pg_map_id + compiler tags).
pub(crate) const MAPPING_WITH_PG_MAP_ID: &str = "\
# compiler: R8
# compiler_version: 8.2.42
# pg_map_id: 1234567
# pg_map_hash: SHA-256 abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890
com.example.android.auth.LoginPresenter -> a.b.c:
    void onLoginClick() -> a
    42:42:void onLoginClick():42:42 -> a
";

/// 200-class synthetic mapping for parse-cost benchmarking.
/// Constant-builder so the bench doesn't pay any allocation
/// cost outside the parser itself.
#[allow(dead_code)]
pub(crate) fn large_mapping() -> String {
    let mut out = String::new();
    for i in 0..200u32 {
        out.push_str(&format!("com.example.pkg.Class{i} -> a{i}.b{i}:\n"));
        for j in 0..10u32 {
            out.push_str(&format!("    void method{j}() -> m{j}\n"));
            out.push_str(&format!(
                "    {start}:{end}:void method{j}():{start}:{end} -> m{j}\n",
                start = 1 + j * 5,
                end = 5 + j * 5,
            ));
        }
    }
    out
}
