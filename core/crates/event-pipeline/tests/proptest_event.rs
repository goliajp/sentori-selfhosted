//! Property tests for Event validation + fingerprint determinism.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use proptest::prelude::*;
use sentori_event_pipeline::{Event, EventKind, IngestService, MessageLevel, Platform};

fn ts() -> time::OffsetDateTime {
    // 2026-01-01 00:00:00Z — fixed so fingerprint stays stable
    // across runs.
    time::OffsetDateTime::from_unix_timestamp(1_767_225_600).expect("ts")
}

proptest! {
    #[test]
    fn fingerprint_is_stable_across_calls(
        error_type in "[A-Z][a-z]{2,16}",
        message in "[a-z ]{1,64}",
        release in "[a-z]{2,8}@[0-9]{1,3}\\.[0-9]{1,3}\\.[0-9]{1,3}",
    ) {
        let event_a = Event::exception(
            uuid::Uuid::now_v7(),
            ts(),
            Platform::Ios,
            release.clone(),
            "production",
            error_type.clone(),
            message.clone(),
        );
        let event_b = Event::exception(
            uuid::Uuid::now_v7(),  // different id — fingerprint should not depend on this
            ts(),
            Platform::Ios,
            release,
            "production",
            error_type,
            message,
        );
        let fp_a = IngestService::fingerprint(&event_a);
        let fp_b = IngestService::fingerprint(&event_b);
        prop_assert_eq!(fp_a, fp_b);
    }

    #[test]
    fn different_release_gives_different_fingerprint(
        release_a in "[a-z]{2,8}@1\\.0\\.0",
        release_b in "[a-z]{2,8}@2\\.0\\.0",
    ) {
        prop_assume!(release_a != release_b);
        let ev_a = Event::exception(
            uuid::Uuid::now_v7(), ts(), Platform::Ios,
            release_a, "prod", "TypeError", "msg",
        );
        let ev_b = Event::exception(
            uuid::Uuid::now_v7(), ts(), Platform::Ios,
            release_b, "prod", "TypeError", "msg",
        );
        prop_assert_ne!(
            IngestService::fingerprint(&ev_a),
            IngestService::fingerprint(&ev_b),
        );
    }

    #[test]
    fn fingerprint_override_used_verbatim(
        // S3's from_override is strict — keep chars + length safe
        // for the validator.
        override_fp in "[a-z0-9_-]{1,32}",
    ) {
        let ev = Event::exception(
            uuid::Uuid::now_v7(), ts(), Platform::Ios,
            "myapp@1.0.0", "prod", "TypeError", "msg",
        ).with_fingerprint_override(override_fp.clone());
        prop_assert_eq!(IngestService::fingerprint(&ev), override_fp);
    }
}

#[test]
fn message_kind_fingerprint_distinct_from_exception() {
    let exc = Event::exception(
        uuid::Uuid::now_v7(),
        ts(),
        Platform::Ios,
        "myapp@1.0.0",
        "prod",
        "TypeError",
        "boom",
    );
    let msg = Event::message(
        uuid::Uuid::now_v7(),
        ts(),
        Platform::Ios,
        "myapp@1.0.0",
        "prod",
        MessageLevel::Error,
        "boom",
    );
    assert_ne!(
        IngestService::fingerprint(&exc),
        IngestService::fingerprint(&msg),
    );
}

#[test]
fn message_body_normalisation_groups_dynamic_ids() {
    // "User 12345 timed out" and "User 67890 timed out" should
    // fingerprint identically thanks to S3's normalize step.
    let a = Event::message(
        uuid::Uuid::now_v7(),
        ts(),
        Platform::Ios,
        "myapp@1.0.0",
        "prod",
        MessageLevel::Warning,
        "User 12345 timed out",
    );
    let b = Event::message(
        uuid::Uuid::now_v7(),
        ts(),
        Platform::Ios,
        "myapp@1.0.0",
        "prod",
        MessageLevel::Warning,
        "User 67890 timed out",
    );
    assert_eq!(
        IngestService::fingerprint(&a),
        IngestService::fingerprint(&b),
    );
}

#[test]
fn event_kind_round_trip() {
    for k in EventKind::ALL {
        let s = k.as_db_str();
        let back = EventKind::from_db_str(s).expect("round trip");
        assert_eq!(back, k);
    }
}

#[test]
fn platform_round_trip() {
    for p in Platform::ALL {
        let s = p.as_db_str();
        let back = Platform::from_db_str(s).expect("round trip");
        assert_eq!(back, p);
    }
}
