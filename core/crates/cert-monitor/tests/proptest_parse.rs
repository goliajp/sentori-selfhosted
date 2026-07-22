//! Property tests for crt.sh timestamp parse + UTF-8 truncate.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use proptest::prelude::*;
use sentori_cert_monitor::CertObservation;

// We re-derive `parse_crt_sh_ts` via a public-test indirection
// — the function is `pub(crate)` to keep callers from
// reaching for it, but the integration test goes through
// the public `poll_domain` path which exercises it end-to-end.
// Here we test the boundary helper `expires_within` instead.

fn sample_obs(not_after: time::OffsetDateTime) -> CertObservation {
    CertObservation {
        id: uuid::Uuid::now_v7(),
        project_id: sentori_workspace_identity::ProjectId::new(),
        domain: "example.com".into(),
        cert_id: 1,
        common_name: None,
        name_value: None,
        issuer_name: "Test CA".into(),
        not_before: time::OffsetDateTime::from_unix_timestamp(0).unwrap(),
        not_after,
        observed_at: time::OffsetDateTime::from_unix_timestamp(0).unwrap(),
    }
}

proptest! {
    #[test]
    fn expires_within_monotonic(days_ahead in 0i64..720) {
        let now = time::OffsetDateTime::from_unix_timestamp(1_767_225_600).unwrap();
        let not_after = now + time::Duration::days(days_ahead);
        let obs = sample_obs(not_after);
        // expires_within(now, days_ahead) should be true (≤).
        prop_assert!(obs.expires_within(now, time::Duration::days(days_ahead)));
        // expires_within(now, days_ahead - 1) is false unless days_ahead == 0.
        if days_ahead > 0 {
            prop_assert!(!obs.expires_within(now, time::Duration::days(days_ahead - 1)));
        }
    }
}

#[test]
fn expires_within_exact_boundary() {
    let now = time::OffsetDateTime::from_unix_timestamp(1_767_225_600).unwrap();
    let obs = sample_obs(now + time::Duration::days(7));
    assert!(obs.expires_within(now, time::Duration::days(7)));
    assert!(obs.expires_within(now, time::Duration::days(8)));
    assert!(!obs.expires_within(now, time::Duration::days(6)));
}
