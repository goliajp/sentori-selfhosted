//! Property tests for `MetricPoint::tags_hash` canonicality.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use proptest::prelude::*;
use sentori_runtime_metrics::MetricPoint;
use sentori_workspace_identity::ProjectId;
use time::OffsetDateTime;

fn fixed_ts() -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(1_767_225_600).unwrap()
}

fn fixed_pid() -> ProjectId {
    ProjectId::new()
}

proptest! {
    #[test]
    fn hash_is_stable_across_insertion_order(
        pairs in proptest::collection::vec(
            ("[a-z]{1,8}", any::<i32>()),
            0..16
        ),
    ) {
        // De-dup keys; serde_json::Map doesn't allow duplicates.
        let mut unique = std::collections::BTreeMap::new();
        for (k, v) in pairs {
            unique.insert(k, v);
        }
        let mut a = MetricPoint::new(fixed_pid(), "m", fixed_ts(), 1.0);
        let mut b = MetricPoint::new(fixed_pid(), "m", fixed_ts(), 1.0);
        // Insert in a order, b in reverse order.
        let mut keys: Vec<_> = unique.keys().cloned().collect();
        for k in &keys {
            a.tags.insert(k.clone(), unique[k].into());
        }
        keys.reverse();
        for k in &keys {
            b.tags.insert(k.clone(), unique[k].into());
        }
        prop_assert_eq!(a.tags_hash(), b.tags_hash());
    }

    #[test]
    fn distinct_tag_maps_distinct_hash(
        a in "[a-z]{2,6}",
        b in "[a-z]{2,6}",
    ) {
        prop_assume!(a != b);
        let mut p_a = MetricPoint::new(fixed_pid(), "m", fixed_ts(), 1.0);
        let mut p_b = MetricPoint::new(fixed_pid(), "m", fixed_ts(), 1.0);
        p_a.tags.insert("k".into(), a.into());
        p_b.tags.insert("k".into(), b.into());
        prop_assert_ne!(p_a.tags_hash(), p_b.tags_hash());
    }

    #[test]
    fn empty_tags_consistent(_seed in 0u8..255) {
        let a = MetricPoint::new(fixed_pid(), "m", fixed_ts(), 1.0);
        let b = MetricPoint::new(fixed_pid(), "m", fixed_ts(), 2.0);
        // value differs, name same, tags both empty → same hash
        prop_assert_eq!(a.tags_hash(), b.tags_hash());
    }
}

#[test]
fn builder_helpers_apply() {
    let pid = ProjectId::new();
    let p = MetricPoint::new(pid, "app.startup_ms", fixed_ts(), 142.0)
        .with_release("v1.0.0")
        .with_environment("production")
        .with_device_class("phone")
        .with_tag("custom", "yes");
    assert_eq!(p.release.as_deref(), Some("v1.0.0"));
    assert_eq!(p.environment.as_deref(), Some("production"));
    assert_eq!(p.device_class.as_deref(), Some("phone"));
    assert_eq!(p.tags.get("custom").and_then(|v| v.as_str()), Some("yes"));
}
