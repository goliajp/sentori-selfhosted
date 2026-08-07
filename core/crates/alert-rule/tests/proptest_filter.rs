//! Property tests for `matches_filter`.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::redundant_clone
)]

use proptest::prelude::*;
use sentori_alert_rule::matches_filter;
use serde_json::json;

proptest! {
    #[test]
    fn empty_filter_always_matches(
        et in "[a-zA-Z]{1,16}",
        env in "[a-z]{1,8}",
        rel in "[a-zA-Z0-9._-]{1,20}",
    ) {
        let f = json!({});
        prop_assert!(matches_filter(&f, &et, &env, &rel));
    }

    #[test]
    fn exact_env_match(want in "[a-z]{1,8}", actual in "[a-z]{1,8}") {
        let f = json!({"environment": &want});
        prop_assert_eq!(matches_filter(&f, "T", &actual, "v1"), want == actual);
    }

    #[test]
    fn exact_release_match(want in "[a-zA-Z0-9._-]{1,16}", actual in "[a-zA-Z0-9._-]{1,16}") {
        let f = json!({"release": &want});
        prop_assert_eq!(matches_filter(&f, "T", "prod", &actual), want == actual);
    }

    #[test]
    fn unknown_filter_keys_are_ignored(
        et in "[a-zA-Z]{1,16}",
        env in "[a-z]{1,8}",
        rel in "[a-zA-Z0-9._-]{1,20}",
        k in "[a-z]{3,12}",
    ) {
        prop_assume!(!matches!(k.as_str(), "environment" | "release" | "errorType"));
        let f = json!({ k.as_str(): "value" });
        prop_assert!(matches_filter(&f, &et, &env, &rel));
    }
}
