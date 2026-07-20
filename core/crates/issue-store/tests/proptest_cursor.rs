//! Property tests for [`Cursor`] round-trip.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use proptest::prelude::*;
use sentori_issue_store::{Cursor, CursorParseError};
use time::OffsetDateTime;
use uuid::Uuid;

proptest! {
    #[test]
    fn first_page_round_trip(limit in 0u32..10_000) {
        let c = Cursor::start(limit);
        let s = c.to_wire_string();
        let back = Cursor::parse(&s).expect("parse");
        prop_assert_eq!(back, c);
        prop_assert!(back.anchor.is_none());
    }

    #[test]
    fn anchored_round_trip(
        ts_secs in 0i64..2_000_000_000,
        id_bytes in proptest::array::uniform16(any::<u8>()),
        limit in 0u32..10_000,
    ) {
        let ts = OffsetDateTime::from_unix_timestamp(ts_secs).expect("ts");
        let id = Uuid::from_bytes(id_bytes);
        let c = Cursor::next(ts, id, limit);
        let s = c.to_wire_string();
        let back = Cursor::parse(&s).expect("parse");
        prop_assert_eq!(back, c);
        prop_assert_eq!(back.anchor, Some((ts, id)));
    }

    #[test]
    fn random_garbage_does_not_panic(s in "[A-Za-z0-9_-]{0,100}") {
        let _ = Cursor::parse(&s); // returns Err — must not panic
    }
}

#[test]
fn invalid_base64() {
    let err = Cursor::parse("!!!!").unwrap_err();
    assert!(matches!(err, CursorParseError::Base64));
}

#[test]
fn limit_clamped() {
    assert_eq!(Cursor::start(0).limit, 1);
    assert_eq!(Cursor::start(99_999).limit, 500);
}
