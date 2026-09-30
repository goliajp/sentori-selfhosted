//! [`Fingerprint`] — the stable group-key hash returned to callers.
//!
//! This module owns the `Fingerprint` newtype, the input enum
//! ([`Input`]), and the constants the public surface exposes
//! ([`OUTPUT_HEX_LEN`], [`MAX_OVERRIDE_LEN`]).

use sha2::{Digest, Sha256};

use crate::ALGO_VERSION_PREFIX;
use crate::error::{FingerprintError, FingerprintResult};
use crate::normalize;

/// Length of a computed fingerprint's hex string. Equal to
/// `16 bytes * 2 hex chars` — the SHA-256 truncation chosen for v1.
pub const OUTPUT_HEX_LEN: usize = 32;

/// Maximum allowed length, in bytes, of a client-supplied override.
///
/// Long fingerprints land in DB index keys, URLs, and notification
/// templates; an unbounded value is an index-bloat / DoS vector.
pub const MAX_OVERRIDE_LEN: usize = 256;

/// In-app frame metadata used for grouping the location of a crash.
///
/// Both fields come from the symbolicated stack frame the application
/// last executed before raising — `function` may be missing when only
/// a file:line is known (anonymous closure, native frame, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameSite<'a> {
    /// Symbol name (e.g. `"renderHeader"`). [`None`] if unavailable.
    pub function: Option<&'a str>,
    /// Source file path or module name (e.g.
    /// `"app/screens/Home.tsx"`).
    pub file: &'a str,
}

/// The components a caller hashes into a fingerprint.
///
/// Each variant corresponds to one of Sentori's event shapes and
/// dictates which fields are required. Wiring code (sentori-server
/// `event-pipeline`, in particular) maps from the on-the-wire event
/// type to the right variant.
///
/// Why an enum, not a single struct of optionals: each shape has a
/// different idea of "what identifies the bug". Forcing all three
/// shapes through one wide struct would let callers silently mix
/// fields (e.g. supplying an `error_type` for a `Message` shape and
/// having it ignored, fragmenting groups in production). The enum
/// makes the choice load-bearing at the type level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Input<'a> {
    /// Manually-captured message (`sentori.captureMessage(...)`).
    ///
    /// Groups by `(release, normalized body)`.
    Message {
        /// Release identifier of the running app. Required even for
        /// messages so a developer-flagged warning in `1.2` and `1.3`
        /// don't collapse together.
        release: &'a str,
        /// The message body the application supplied.
        body: &'a str,
    },

    /// A captured exception, ANR, native crash, or near-crash.
    ///
    /// Groups by `(release, error_type, normalized message, frame
    /// site)`. The frame site is the first in-app frame the server
    /// could resolve; pass [`None`] when no in-app frame exists and
    /// the top stack frame is also unavailable.
    Exception {
        /// Release identifier of the running app.
        release: &'a str,
        /// Exception class / kind tag (e.g. `"TypeError"`,
        /// `"java.lang.NullPointerException"`).
        error_type: &'a str,
        /// The exception's human message.
        ///
        /// Normalised via [`crate::normalize::message`] before
        /// hashing so dynamic identifiers don't fragment groups.
        message: &'a str,
        /// Identifying frame, when known.
        frame: Option<FrameSite<'a>>,
    },

    /// Event with neither a body nor an exception — degenerate but
    /// still ingest-accepted.
    ///
    /// Hashes on `(release, kind tag, seed)` so each such event lands
    /// in its own group rather than collapsing every degenerate event
    /// into one. `seed` is typically the event timestamp (Unix
    /// seconds) but any caller-stable value works.
    Degenerate {
        /// Release identifier of the running app.
        release: &'a str,
        /// A short label the server uses internally to record what
        /// kind of degenerate event this was (e.g. `"anr"`,
        /// `"near_crash"`).
        kind_tag: &'a str,
        /// Per-event uniqueness seed.
        seed: i64,
    },
}

/// A 32-char hex fingerprint identifying which issue group an event
/// belongs to.
///
/// Cloning is cheap (a [`String`]). The hex form is the canonical
/// representation — equality and ordering both delegate to it, so
/// comparisons match what a downstream DB would see.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Fingerprint(String);

impl Fingerprint {
    /// Compute the fingerprint for the given input components.
    ///
    /// Pure function; the same input always produces the same
    /// fingerprint and no input shape is rejected.
    #[must_use]
    pub fn compute(input: &Input<'_>) -> Self {
        let mut h = Sha256::new();
        h.update(ALGO_VERSION_PREFIX);

        match input {
            Input::Message { release, body } => {
                h.update(b"message|");
                h.update(release.as_bytes());
                h.update(b"|");
                h.update(normalize::message(body).as_bytes());
            }
            Input::Exception {
                release,
                error_type,
                message,
                frame,
            } => {
                h.update(b"exception|");
                h.update(release.as_bytes());
                h.update(b"|");
                h.update(error_type.as_bytes());
                h.update(b"|");
                h.update(normalize::message(message).as_bytes());
                h.update(b"|");
                if let Some(site) = frame {
                    if let Some(fn_name) = site.function {
                        h.update(fn_name.as_bytes());
                    }
                    h.update(b"|");
                    h.update(site.file.as_bytes());
                }
            }
            Input::Degenerate {
                release,
                kind_tag,
                seed,
            } => {
                h.update(b"degenerate|");
                h.update(release.as_bytes());
                h.update(b"|");
                h.update(kind_tag.as_bytes());
                h.update(b"|");
                h.update(seed.to_be_bytes());
            }
        }

        let digest = h.finalize();
        Self(hex::encode(&digest[..16]))
    }

    /// Build a fingerprint from a caller-supplied override string.
    ///
    /// Overrides bypass the internal grouping logic entirely — the
    /// supplied string becomes the fingerprint verbatim (after
    /// validation) so the caller can express custom grouping
    /// (`"payment.card-decline"`, `"feature-flag.X"`, …).
    ///
    /// # Errors
    ///
    /// - [`FingerprintError::OverrideEmpty`] if `s` is empty.
    /// - [`FingerprintError::OverrideTooLong`] if `s` exceeds
    ///   [`MAX_OVERRIDE_LEN`] bytes.
    /// - [`FingerprintError::OverrideControlChar`] if `s` contains a
    ///   control byte (`0x00..=0x1F` or `0x7F`).
    pub fn from_override(s: &str) -> FingerprintResult<Self> {
        if s.is_empty() {
            return Err(FingerprintError::OverrideEmpty);
        }
        if s.len() > MAX_OVERRIDE_LEN {
            return Err(FingerprintError::OverrideTooLong {
                got: s.len(),
                max: MAX_OVERRIDE_LEN,
            });
        }
        if let Some((idx, _)) = s
            .as_bytes()
            .iter()
            .enumerate()
            .find(|&(_, &b)| b.is_ascii_control())
        {
            return Err(FingerprintError::OverrideControlChar { at: idx });
        }
        Ok(Self(s.to_owned()))
    }

    /// Hex view of the fingerprint.
    ///
    /// For [`Fingerprint::compute`] outputs this is exactly
    /// [`OUTPUT_HEX_LEN`] lowercase hex chars. For
    /// [`Fingerprint::from_override`] outputs it is the validated
    /// override string verbatim.
    #[must_use]
    pub fn as_hex(&self) -> &str {
        &self.0
    }

    /// Consume into the underlying [`String`].
    #[must_use]
    pub fn into_hex(self) -> String {
        self.0
    }
}

impl core::fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for Fingerprint {
    fn as_ref(&self) -> &str {
        &self.0
    }
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

    fn msg(release: &str, body: &str) -> Fingerprint {
        Fingerprint::compute(&Input::Message { release, body })
    }

    fn exc<'a>(
        release: &'a str,
        error_type: &'a str,
        message: &'a str,
        frame: Option<FrameSite<'a>>,
    ) -> Fingerprint {
        Fingerprint::compute(&Input::Exception {
            release,
            error_type,
            message,
            frame,
        })
    }

    // ---------- output shape ----------

    #[test]
    fn computed_output_is_32_lowercase_hex() {
        let fp = msg("v1", "hi");
        let s = fp.as_hex();
        assert_eq!(s.len(), OUTPUT_HEX_LEN);
        assert!(
            s.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
    }

    #[test]
    fn display_and_as_ref_match_hex() {
        let fp = msg("v1", "hi");
        assert_eq!(format!("{fp}"), fp.as_hex());
        assert_eq!(<Fingerprint as AsRef<str>>::as_ref(&fp), fp.as_hex());
        assert_eq!(fp.clone().into_hex(), fp.as_hex());
    }

    // ---------- determinism ----------

    #[test]
    fn message_is_deterministic() {
        assert_eq!(msg("1.0", "boom"), msg("1.0", "boom"));
    }

    #[test]
    fn exception_is_deterministic() {
        let f = FrameSite {
            function: Some("foo"),
            file: "a/b.rs",
        };
        assert_eq!(
            exc("1.0", "E", "boom", Some(f)),
            exc("1.0", "E", "boom", Some(f)),
        );
    }

    #[test]
    fn degenerate_is_deterministic() {
        let d = || {
            Fingerprint::compute(&Input::Degenerate {
                release: "v1",
                kind_tag: "anr",
                seed: 42,
            })
        };
        assert_eq!(d(), d());
    }

    // ---------- field-level isolation ----------

    #[test]
    fn message_release_isolation() {
        assert_ne!(msg("1.0", "boom"), msg("1.1", "boom"));
    }

    #[test]
    fn message_body_isolation() {
        assert_ne!(msg("1.0", "boom-a"), msg("1.0", "boom-b"));
    }

    #[test]
    fn exception_release_isolation() {
        assert_ne!(exc("1.0", "E", "m", None), exc("1.1", "E", "m", None),);
    }

    #[test]
    fn exception_type_isolation() {
        assert_ne!(exc("1.0", "E1", "m", None), exc("1.0", "E2", "m", None),);
    }

    #[test]
    fn exception_message_isolation() {
        assert_ne!(exc("1.0", "E", "m1", None), exc("1.0", "E", "m2", None),);
    }

    #[test]
    fn exception_frame_function_isolation() {
        let f1 = FrameSite {
            function: Some("a"),
            file: "x.rs",
        };
        let f2 = FrameSite {
            function: Some("b"),
            file: "x.rs",
        };
        assert_ne!(
            exc("1.0", "E", "m", Some(f1)),
            exc("1.0", "E", "m", Some(f2)),
        );
    }

    #[test]
    fn exception_frame_file_isolation() {
        let f1 = FrameSite {
            function: Some("a"),
            file: "x.rs",
        };
        let f2 = FrameSite {
            function: Some("a"),
            file: "y.rs",
        };
        assert_ne!(
            exc("1.0", "E", "m", Some(f1)),
            exc("1.0", "E", "m", Some(f2)),
        );
    }

    #[test]
    fn exception_no_frame_is_distinct_from_empty_frame() {
        // No frame vs (None, "") — still meaningful difference.
        let empty = FrameSite {
            function: None,
            file: "",
        };
        assert_ne!(
            exc("1.0", "E", "m", None),
            exc("1.0", "E", "m", Some(empty)),
        );
    }

    // ---------- kind isolation ----------

    #[test]
    fn message_and_exception_with_same_release_differ() {
        // Same release+body matched against an exception-shape with
        // the body shoved into `message` must still differ — kind tag
        // is part of the hash.
        assert_ne!(msg("1.0", "boom"), exc("1.0", "boom", "boom", None),);
    }

    #[test]
    fn degenerate_seed_isolation() {
        let a = Fingerprint::compute(&Input::Degenerate {
            release: "v1",
            kind_tag: "anr",
            seed: 1,
        });
        let b = Fingerprint::compute(&Input::Degenerate {
            release: "v1",
            kind_tag: "anr",
            seed: 2,
        });
        assert_ne!(a, b);
    }

    // ---------- normalisation interplay ----------

    #[test]
    fn dynamic_ids_dont_fragment_message_grouping() {
        assert_eq!(
            msg("1.0", "User 12345 timed out"),
            msg("1.0", "User 67890 timed out"),
        );
    }

    #[test]
    fn dynamic_ids_dont_fragment_exception_grouping() {
        assert_eq!(
            exc("1.0", "E", "rec=1234 failed", None),
            exc("1.0", "E", "rec=5678 failed", None),
        );
    }

    #[test]
    fn uuid_in_message_collapses() {
        assert_eq!(
            msg("1.0", "sid=7f3b1c8a-2e3d-4f5a-9b0c-1d2e3f4a5b6c failed"),
            msg("1.0", "sid=11111111-2222-3333-4444-555555555555 failed"),
        );
    }

    // ---------- override path ----------

    #[test]
    fn override_is_verbatim() {
        let fp = Fingerprint::from_override("payment.card-decline").unwrap();
        assert_eq!(fp.as_hex(), "payment.card-decline");
    }

    #[test]
    fn override_empty_rejected() {
        assert_eq!(
            Fingerprint::from_override(""),
            Err(FingerprintError::OverrideEmpty)
        );
    }

    #[test]
    fn override_too_long_rejected() {
        let s = "a".repeat(MAX_OVERRIDE_LEN + 1);
        assert_eq!(
            Fingerprint::from_override(&s),
            Err(FingerprintError::OverrideTooLong {
                got: MAX_OVERRIDE_LEN + 1,
                max: MAX_OVERRIDE_LEN,
            })
        );
    }

    #[test]
    fn override_max_len_accepted() {
        let s = "a".repeat(MAX_OVERRIDE_LEN);
        assert!(Fingerprint::from_override(&s).is_ok());
    }

    #[test]
    fn override_control_char_rejected() {
        let bad = "ok\tnope";
        let err = Fingerprint::from_override(bad).unwrap_err();
        assert_eq!(err, FingerprintError::OverrideControlChar { at: 2 });

        let with_null = "ok\0";
        let err2 = Fingerprint::from_override(with_null).unwrap_err();
        assert_eq!(err2, FingerprintError::OverrideControlChar { at: 2 });
    }

    #[test]
    fn override_allows_printable_punctuation() {
        for ok in [
            "category:thing-1",
            "team/payments::card-decline",
            "abc.def.ghi",
            "with spaces ok",
        ] {
            assert!(Fingerprint::from_override(ok).is_ok(), "{ok}");
        }
    }

    // ---------- ordering / collections ----------

    #[test]
    fn fingerprint_orders_lexicographically() {
        let mut v = [msg("1.0", "b"), msg("1.0", "a"), msg("1.0", "c")];
        v.sort();
        for w in v.windows(2) {
            assert!(w[0] <= w[1]);
        }
    }
}
