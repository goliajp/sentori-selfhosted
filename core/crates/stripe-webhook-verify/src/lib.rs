//! # `sentori-stripe-webhook-verify` — Stripe webhook signature verifier
//!
//! Stone-tier crate (per cement-stone methodology) that implements
//! Stripe's documented `v1` webhook signature scheme directly using
//! `hmac` + `sha2` + `subtle`. Does NOT depend on `async-stripe` —
//! that dependency lives in the saas 水泥 layer where the broader
//! Stripe SDK is actually needed, and pulling it into a stone would
//! drag in tokio, hyper-rustls, and the full Stripe domain model
//! for what is fundamentally a 30-line HMAC verification.
//!
//! ## The Stripe `v1` signature scheme
//!
//! Stripe sends every webhook with a `Stripe-Signature` header
//! shaped like
//!
//! ```text
//! t=1733567890,v1=abcdef…01,v1=ffeedd…99
//! ```
//!
//! The receiver must:
//!
//! 1. Parse the header into a timestamp `t` and one or more `v1`
//!    hex-encoded signatures (multiple appear during webhook
//!    secret rotation).
//! 2. Build the signed payload as `<t> || "." || <raw body>`.
//! 3. Compute `expected = HMAC-SHA256(secret, signed_payload)`.
//! 4. Accept iff `expected` matches *any* of the `v1=` values under
//!    constant-time comparison, AND `|now - t| <= tolerance`.
//!
//! The freshness window prevents replays of an otherwise-valid old
//! event. Stripe's recommended default is 5 minutes; this crate
//! exposes it as a per-call parameter so dogfood replay tools can
//! widen the window when intentionally re-injecting historical
//! events.
//!
//! ## Quick start
//!
//! ```rust
//! use sentori_stripe_webhook_verify::{verify, Tolerance};
//!
//! # fn demo(
//! #     secret: &[u8],
//! #     header: &str,
//! #     body: &[u8],
//! #     now_unix: i64,
//! # ) -> Result<(), sentori_stripe_webhook_verify::VerifyError> {
//! let verified = verify(secret, header, body, now_unix, Tolerance::default())?;
//! println!("event timestamp: {}", verified.timestamp);
//! # Ok(())
//! # }
//! ```
//!
//! ## Security stance
//!
//! - The HMAC comparison is constant-time via [`subtle::ConstantTimeEq`].
//! - The crate rejects empty signature lists, mis-formatted
//!   timestamps, non-hex `v1=` values, and out-of-window timestamps
//!   distinctly so callers can log diagnostics without leaking
//!   secret state.
//! - The header parser tolerates whitespace and the documented
//!   alternate schemes (`v0=`) by ignoring unknown scheme tags —
//!   only `v1` is consumed for the accept decision, matching
//!   Stripe's published guidance.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
// Crate docs interleave English narrative with technical pseudocode
// the doc_markdown heuristic mis-flags ("HMAC", "SHA-256", "URLs").
#![allow(clippy::doc_markdown)]

mod error;
mod verifier;

pub use error::{VerifyError, VerifyResult};
pub use verifier::{DEFAULT_TOLERANCE_SECS, Tolerance, Verified, verify};
