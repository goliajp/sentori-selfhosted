//! # `sentori-issue-fingerprint` — stable group-key hash for events
//!
//! Stone-tier crate (per cement-stone methodology) that turns the
//! identifying components of a captured event into a deterministic
//! 32-char hex fingerprint. Sentori uses the fingerprint as the dedup
//! key for the [issue table]: every accepted event is fingerprinted and
//! events that share a fingerprint roll up into one Issue row.
//!
//! [issue table]: https://sentori.golia.jp/concept/issue/
//!
//! ## Algorithm (locked v1)
//!
//! ```text
//! digest   = SHA256(
//!     b"sentori-issue-fingerprint:v1:"  // version-tag prefix
//!     || kind_tag                        // "message" / "exception" / "degenerate"
//!     || b"|" || components(...)         // type-specific fields, separator-delimited
//! )
//! output   = hex(digest[..16])           // 32 lowercase hex chars
//! ```
//!
//! The version-tag prefix is part of the hash input so a future
//! algorithm change can never silently produce the same fingerprint
//! as v1 — old and new fingerprints can coexist behind the version
//! tag.
//!
//! Truncating SHA-256 to 16 bytes keeps the fingerprint legible inside
//! URLs and DB rows while preserving ~64-bit collision resistance —
//! comfortably above the working-set size of any plausible Sentori
//! issue table (millions of distinct groups, not billions).
//!
//! ## Grouping policy (Sentori's deliberate divergence)
//!
//! The components hashed for each event kind encode three product
//! choices the project re-evaluated post-v1.x:
//!
//! - **Per-release isolation**, not cross-release regression flip.
//!   A bug seen in `5.3` and a bug seen in `5.4` are two issues, not
//!   one issue that flips `resolved → regressed`. The dashboard can
//!   answer "did a fixed bug come back" via a related-issues panel
//!   instead. → `release` is in every hash input.
//!
//! - **Different exception messages split.**
//!   `"pinning mismatch (mode=block)"` and
//!   `"pinning mismatch (mode=alert-only)"` on the same callsite are
//!   functionally different conditions — block vs alert-only is a
//!   behaviour split, not the same bug. v1.x collapsed both into one
//!   issue and made triage impossible. → normalized message is in
//!   every error-shape hash input.
//!
//! - **Dynamic IDs don't fragment.** [`normalize::message`] strips
//!   digit runs of length ≥ 4 and full UUIDs so
//!   `"User 12345 timed out"` and `"User 67890 timed out"` still
//!   group together — same condition, different identifier.
//!
//! ## Quick start
//!
//! ```rust
//! use sentori_issue_fingerprint::{Fingerprint, FrameSite, Input};
//!
//! // An exception captured from the running app.
//! let fp = Fingerprint::compute(&Input::Exception {
//!     release: "myapp@5.3.1",
//!     error_type: "TypeError",
//!     message: "Cannot read property 'id' of undefined (user 12345)",
//!     frame: Some(FrameSite {
//!         function: Some("renderHeader"),
//!         file: "app/screens/Home.tsx",
//!     }),
//! });
//!
//! assert_eq!(fp.as_hex().len(), 32);
//! assert!(fp.as_hex().chars().all(|c| c.is_ascii_hexdigit()));
//!
//! // Same callsite, different dynamic id → same group.
//! let same = Fingerprint::compute(&Input::Exception {
//!     release: "myapp@5.3.1",
//!     error_type: "TypeError",
//!     message: "Cannot read property 'id' of undefined (user 67890)",
//!     frame: Some(FrameSite {
//!         function: Some("renderHeader"),
//!         file: "app/screens/Home.tsx",
//!     }),
//! });
//! assert_eq!(fp, same);
//! ```
//!
//! ## Client overrides
//!
//! Clients can short-circuit the internal grouping by supplying their
//! own fingerprint string — used when the application has business
//! context the server cannot infer (e.g. "all card-decline errors are
//! one issue regardless of message"). Validate the override before
//! storing it:
//!
//! ```rust
//! use sentori_issue_fingerprint::Fingerprint;
//!
//! let fp = Fingerprint::from_override("payment.card-decline")?;
//! assert_eq!(fp.as_hex(), "payment.card-decline");
//! # Ok::<(), sentori_issue_fingerprint::FingerprintError>(())
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]
// Crate docs interleave English prose with technical pseudocode that
// the doc_markdown heuristic mis-flags ("SHA-256", "URLs", etc.).
#![allow(clippy::doc_markdown)]

mod error;
mod fingerprint;
pub mod normalize;

pub use error::{FingerprintError, FingerprintResult};
pub use fingerprint::{Fingerprint, FrameSite, Input, MAX_OVERRIDE_LEN, OUTPUT_HEX_LEN};

/// Versioned prefix mixed into the SHA-256 input — bump when the
/// algorithm changes so v1 and v2 fingerprints cannot collide.
pub const ALGO_VERSION_PREFIX: &[u8] = b"sentori-issue-fingerprint:v1:";
