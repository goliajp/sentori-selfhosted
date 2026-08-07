//! Proptests for [`ProviderKind`] round-trip + [`SendOutcome`]
//! invariants.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use proptest::prelude::*;
use sentori_push_provider::{ProviderKind, SendOutcome};

fn any_kind() -> impl Strategy<Value = ProviderKind> {
    prop_oneof![
        Just(ProviderKind::Apns),
        Just(ProviderKind::Fcm),
        Just(ProviderKind::WebPush),
        Just(ProviderKind::Hcm),
        Just(ProviderKind::MiPush),
    ]
}

proptest! {
    #[test]
    fn kind_db_str_round_trip(k in any_kind()) {
        let s = k.as_db_str();
        prop_assert_eq!(ProviderKind::from_db_str(s).unwrap(), k);
    }

    #[test]
    fn parse_rejects_unknown(s in "[a-z]{1,8}") {
        prop_assume!(!["apns","fcm","webpush","hcm","mipush"].contains(&s.as_str()));
        prop_assert!(ProviderKind::from_db_str(&s).is_err());
    }

    #[test]
    fn serde_round_trip_outcome(retry_after in proptest::option::of(0i32..3600)) {
        // Pick a Transient outcome with optional retry hint;
        // serde_json round-trip must preserve.
        let o = SendOutcome::Transient {
            retry_after_secs: retry_after,
        };
        let json = serde_json::to_string(&o).expect("ser");
        let back: SendOutcome = serde_json::from_str(&json).expect("de");
        prop_assert_eq!(back, o);
    }
}

#[test]
fn quarantine_only_on_permanently_invalid() {
    assert!(SendOutcome::PermanentlyInvalidToken.should_quarantine());
    assert!(!SendOutcome::Sent.should_quarantine());
    assert!(!SendOutcome::EnvironmentMismatch.should_quarantine());
    assert!(
        !SendOutcome::Transient {
            retry_after_secs: None
        }
        .should_quarantine()
    );
    assert!(!SendOutcome::TerminalOther { reason: "x".into() }.should_quarantine());
}

#[test]
fn retryable_set() {
    assert!(!SendOutcome::Sent.is_retryable(), "sent isn't retryable");
    assert!(SendOutcome::EnvironmentMismatch.is_retryable());
    assert!(
        SendOutcome::Transient {
            retry_after_secs: None
        }
        .is_retryable()
    );
    assert!(!SendOutcome::PermanentlyInvalidToken.is_retryable());
}

#[test]
fn kind_all_includes_every_variant() {
    let all = ProviderKind::ALL;
    assert_eq!(all.len(), 5);
    for k in [
        ProviderKind::Apns,
        ProviderKind::Fcm,
        ProviderKind::WebPush,
        ProviderKind::Hcm,
        ProviderKind::MiPush,
    ] {
        assert!(all.contains(&k));
    }
}
