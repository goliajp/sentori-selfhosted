//! Property tests for [`SpanStatus::worst_of`] commutativity
//! + [`Cursor`] round-trip (already tested in K5; smoke here).

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use proptest::prelude::*;
use sentori_span_store::SpanStatus;

fn any_status() -> impl Strategy<Value = SpanStatus> {
    prop_oneof![
        Just(SpanStatus::Ok),
        Just(SpanStatus::Error),
        Just(SpanStatus::Cancelled),
    ]
}

proptest! {
    #[test]
    fn worst_of_is_commutative(a in any_status(), b in any_status()) {
        prop_assert_eq!(a.worst_of(b), b.worst_of(a));
    }

    #[test]
    fn worst_of_idempotent(a in any_status()) {
        prop_assert_eq!(a.worst_of(a), a);
    }

    #[test]
    fn error_dominates_everything(other in any_status()) {
        prop_assert_eq!(SpanStatus::Error.worst_of(other), SpanStatus::Error);
        prop_assert_eq!(other.worst_of(SpanStatus::Error), SpanStatus::Error);
    }

    #[test]
    fn cancelled_beats_ok(b in any_status()) {
        let out = SpanStatus::Cancelled.worst_of(b);
        if b == SpanStatus::Error {
            prop_assert_eq!(out, SpanStatus::Error);
        } else {
            prop_assert_eq!(out, SpanStatus::Cancelled);
        }
    }

    #[test]
    fn db_str_round_trip(s in any_status()) {
        let back = SpanStatus::from_db_str(s.as_db_str()).unwrap();
        prop_assert_eq!(back, s);
    }
}
