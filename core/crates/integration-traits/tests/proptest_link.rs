//! Property tests for the [`ExternalRef`] + lifecycle event
//! basics.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::redundant_clone
)]

use proptest::prelude::*;
use sentori_integration_traits::{ConnectMode, ExternalRef, IssueLifecycleEvent};

proptest! {
    #[test]
    fn external_ref_eq_self_clone(id in "[a-zA-Z0-9_-]{1,32}", url in "https?://[a-z]{3,10}\\.com/[a-z]{1,10}") {
        let a = ExternalRef { external_id: id.clone(), external_url: url.clone() };
        let b = a.clone();
        prop_assert_eq!(a, b);
    }

    #[test]
    fn lifecycle_serde_round_trip(
        e in prop_oneof![
            Just(IssueLifecycleEvent::Created),
            Just(IssueLifecycleEvent::Regressed),
            Just(IssueLifecycleEvent::Resolved),
        ],
    ) {
        let s = serde_json::to_string(&e).unwrap();
        let back: IssueLifecycleEvent = serde_json::from_str(&s).unwrap();
        prop_assert_eq!(back, e);
    }

    #[test]
    fn connect_mode_serde_round_trip(
        m in prop_oneof![Just(ConnectMode::OAuth), Just(ConnectMode::Manual)],
    ) {
        let s = serde_json::to_string(&m).unwrap();
        let back: ConnectMode = serde_json::from_str(&s).unwrap();
        prop_assert_eq!(back, m);
    }
}
