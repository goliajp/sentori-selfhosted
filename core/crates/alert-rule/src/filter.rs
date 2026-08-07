//! Filter matching — pure function shape so callers can
//! reuse for previewing rules ("would this fire?") without
//! touching the DB.

use serde_json::Value;

/// True when `filter` JSON matches the given event
/// dimensions. All fields are optional; absent = no
/// constraint. Exact-match semantics (no regex in v0.1).
///
/// Recognised keys (camelCase):
/// - `environment`: exact-match string against `environment`
/// - `release`: exact-match string against `release`
/// - `errorType`: exact-match string against `error_type`
///
/// Unknown keys are ignored (defensive — future rule
/// versions can add keys and old K14 versions will silently
/// match-permissively rather than reject-and-drop).
#[must_use]
pub fn matches_filter(filter: &Value, error_type: &str, environment: &str, release: &str) -> bool {
    if let Some(want) = filter.get("environment").and_then(Value::as_str)
        && want != environment
    {
        return false;
    }
    if let Some(want) = filter.get("release").and_then(Value::as_str)
        && want != release
    {
        return false;
    }
    if let Some(want) = filter.get("errorType").and_then(Value::as_str)
        && want != error_type
    {
        return false;
    }
    true
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn empty_filter_matches_anything() {
        assert!(matches_filter(&json!({}), "x", "prod", "v1"));
        assert!(matches_filter(&Value::Null, "x", "prod", "v1"));
    }

    #[test]
    fn env_match_required_when_set() {
        let f = json!({"environment": "production"});
        assert!(matches_filter(&f, "T", "production", "v1"));
        assert!(!matches_filter(&f, "T", "staging", "v1"));
    }

    #[test]
    fn release_match_required_when_set() {
        let f = json!({"release": "v1.0.0"});
        assert!(matches_filter(&f, "T", "prod", "v1.0.0"));
        assert!(!matches_filter(&f, "T", "prod", "v2.0.0"));
    }

    #[test]
    fn error_type_match_required_when_set() {
        let f = json!({"errorType": "TypeError"});
        assert!(matches_filter(&f, "TypeError", "prod", "v1"));
        assert!(!matches_filter(&f, "NullDeref", "prod", "v1"));
    }

    #[test]
    fn all_three_must_match() {
        let f = json!({
            "environment": "production",
            "release": "v1",
            "errorType": "T",
        });
        assert!(matches_filter(&f, "T", "production", "v1"));
        assert!(!matches_filter(&f, "T", "staging", "v1"));
        assert!(!matches_filter(&f, "T", "production", "v2"));
        assert!(!matches_filter(&f, "U", "production", "v1"));
    }

    #[test]
    fn unknown_keys_ignored() {
        let f = json!({"futureKey": "anything"});
        assert!(matches_filter(&f, "T", "prod", "v1"));
    }
}
