//! Property tests asserting fingerprint stability + per-component
//! isolation. Any flake in determinism, or any cross-component leak
//! (changing field X leaves the fingerprint unchanged), would silently
//! shred Sentori's grouping in production.

#![allow(
    missing_docs,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_panics_doc
)]

use proptest::prelude::*;
use sentori_issue_fingerprint::{Fingerprint, FrameSite, Input, MAX_OVERRIDE_LEN, OUTPUT_HEX_LEN};

fn arb_release() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_.@+\\-]{1,32}".prop_map(Into::into)
}

fn arb_body() -> impl Strategy<Value = String> {
    // Restrict to printable ASCII so the comparison logic stays clear;
    // unicode normalisation is exercised in the unit tests.
    "[A-Za-z0-9 :=\\-_.,/<>]{0,128}".prop_map(Into::into)
}

fn arb_error_type() -> impl Strategy<Value = String> {
    "[A-Za-z][A-Za-z0-9.]{0,40}".prop_map(Into::into)
}

fn arb_function() -> impl Strategy<Value = Option<String>> {
    proptest::option::of("[a-zA-Z_][a-zA-Z0-9_]{0,32}".prop_map(Into::into))
}

fn arb_file() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_/\\.]{1,64}".prop_map(Into::into)
}

fn arb_frame() -> impl Strategy<Value = Option<(Option<String>, String)>> {
    proptest::option::of((arb_function(), arb_file()))
}

fn compute_exception(
    release: &str,
    error_type: &str,
    message: &str,
    frame: Option<&(Option<String>, String)>,
) -> Fingerprint {
    let frame_site = frame.map(|(fn_name, file)| FrameSite {
        function: fn_name.as_deref(),
        file: file.as_str(),
    });
    Fingerprint::compute(&Input::Exception {
        release,
        error_type,
        message,
        frame: frame_site,
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(192))]

    // ---------- output shape ----------

    #[test]
    fn message_output_is_32_hex(
        release in arb_release(),
        body in arb_body(),
    ) {
        let fp = Fingerprint::compute(&Input::Message {
            release: &release,
            body: &body,
        });
        prop_assert_eq!(fp.as_hex().len(), OUTPUT_HEX_LEN);
        prop_assert!(fp.as_hex().chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn exception_output_is_32_hex(
        release in arb_release(),
        error_type in arb_error_type(),
        message in arb_body(),
        frame in arb_frame(),
    ) {
        let fp = compute_exception(&release, &error_type, &message, frame.as_ref());
        prop_assert_eq!(fp.as_hex().len(), OUTPUT_HEX_LEN);
        prop_assert!(fp.as_hex().chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn degenerate_output_is_32_hex(
        release in arb_release(),
        kind_tag in "[a-z_]{1,16}",
        seed in any::<i64>(),
    ) {
        let fp = Fingerprint::compute(&Input::Degenerate {
            release: &release,
            kind_tag: &kind_tag,
            seed,
        });
        prop_assert_eq!(fp.as_hex().len(), OUTPUT_HEX_LEN);
        prop_assert!(fp.as_hex().chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    // ---------- determinism ----------

    #[test]
    fn message_determinism(
        release in arb_release(),
        body in arb_body(),
    ) {
        let a = Fingerprint::compute(&Input::Message {
            release: &release,
            body: &body,
        });
        let b = Fingerprint::compute(&Input::Message {
            release: &release,
            body: &body,
        });
        prop_assert_eq!(a, b);
    }

    #[test]
    fn exception_determinism(
        release in arb_release(),
        error_type in arb_error_type(),
        message in arb_body(),
        frame in arb_frame(),
    ) {
        let a = compute_exception(&release, &error_type, &message, frame.as_ref());
        let b = compute_exception(&release, &error_type, &message, frame.as_ref());
        prop_assert_eq!(a, b);
    }

    // ---------- isolation ----------

    #[test]
    fn message_release_isolation(
        r1 in arb_release(),
        r2 in arb_release(),
        body in arb_body(),
    ) {
        prop_assume!(r1 != r2);
        prop_assert_ne!(
            Fingerprint::compute(&Input::Message { release: &r1, body: &body }),
            Fingerprint::compute(&Input::Message { release: &r2, body: &body }),
        );
    }

    #[test]
    fn message_body_isolation(
        release in arb_release(),
        b1 in arb_body(),
        b2 in arb_body(),
    ) {
        // Normalisation may collapse two bodies that only differ in
        // dynamic identifiers — assume away those by comparing the
        // post-normalisation strings.
        use sentori_issue_fingerprint::normalize;
        prop_assume!(normalize::message(&b1) != normalize::message(&b2));
        prop_assert_ne!(
            Fingerprint::compute(&Input::Message { release: &release, body: &b1 }),
            Fingerprint::compute(&Input::Message { release: &release, body: &b2 }),
        );
    }

    #[test]
    fn exception_error_type_isolation(
        release in arb_release(),
        e1 in arb_error_type(),
        e2 in arb_error_type(),
        message in arb_body(),
        frame in arb_frame(),
    ) {
        prop_assume!(e1 != e2);
        prop_assert_ne!(
            compute_exception(&release, &e1, &message, frame.as_ref()),
            compute_exception(&release, &e2, &message, frame.as_ref()),
        );
    }

    #[test]
    fn kind_tag_isolation(
        release in arb_release(),
        body in arb_body(),
    ) {
        // A message-shaped fingerprint and an exception-shaped one
        // with the same release+body strings must not collide — the
        // kind tag is part of the hash.
        let m = Fingerprint::compute(&Input::Message {
            release: &release,
            body: &body,
        });
        let e = Fingerprint::compute(&Input::Exception {
            release: &release,
            error_type: &body, // intentionally mirror body
            message: &body,
            frame: None,
        });
        prop_assert_ne!(m, e);
    }

    #[test]
    fn degenerate_seed_isolation(
        release in arb_release(),
        kind_tag in "[a-z]{1,8}",
        s1 in any::<i64>(),
        s2 in any::<i64>(),
    ) {
        prop_assume!(s1 != s2);
        prop_assert_ne!(
            Fingerprint::compute(&Input::Degenerate {
                release: &release,
                kind_tag: &kind_tag,
                seed: s1,
            }),
            Fingerprint::compute(&Input::Degenerate {
                release: &release,
                kind_tag: &kind_tag,
                seed: s2,
            }),
        );
    }

    // ---------- override path ----------

    #[test]
    fn override_round_trip(s in "[A-Za-z0-9 :=._/+\\-]{1,256}") {
        let fp = Fingerprint::from_override(&s).unwrap();
        prop_assert_eq!(fp.as_hex(), &s);
    }

    #[test]
    fn override_rejects_too_long(s in "[A-Za-z]{257,400}") {
        prop_assume!(s.len() > MAX_OVERRIDE_LEN);
        prop_assert!(Fingerprint::from_override(&s).is_err());
    }
}
