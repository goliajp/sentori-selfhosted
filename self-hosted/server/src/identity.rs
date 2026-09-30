//! The identity key, computed the way the SDKs compute it.
//!
//! A caller says "notify user usr_123". The device rows carry
//! `user_key`, which is a hash — so answering that request means
//! hashing the raw id the same way the device did, or the join
//! matches nothing and the notification quietly reaches no one.
//!
//! ## This is the fourth implementation
//!
//! TypeScript, Kotlin and Swift already compute it, and
//! `SentoriIdentity.kt` says why that is dangerous:
//!
//! > They must agree byte for byte. If they drift, a device stops
//! > matching the events from the same person: the push reaches
//! > nobody and nothing reports it, which is precisely why a second
//! > implementation is a bad idea unless something is holding the
//! > three together.
//!
//! That something is `sdk/native/fixtures/identity-vectors.json`,
//! generated from the TypeScript source of truth. This file is held
//! by the same fixture, in the test at the bottom — a fourth
//! implementation with no gate would be exactly the mistake the
//! comment warns about, written by someone who had read it.
//!
//! ## Unsalted, on purpose
//!
//! The client hash is a plain SHA-256 of the normalised value. The
//! server layers a per-scope salt elsewhere, for the fingerprint
//! table; several comments in this codebase call the client value
//! "salted" and they are wrong.

use sha2::{Digest, Sha256};

/// What every implementation strips.
///
/// Kotlin's `trim()` counts neither U+00A0 nor U+FEFF; ECMAScript
/// counts both; Swift's `.whitespacesAndNewlines` counts the first
/// and not the second. A value pasted out of a web page routinely
/// carries them, and any of those defaults makes one device hash
/// differently from its own events — for one user, silently.
fn trim_like_the_others(raw: &str) -> &str {
    raw.trim_matches(|c: char| c.is_whitespace() || c == '\u{00a0}' || c == '\u{feff}')
}

/// The normalisation for a key type, mirroring `normalise` in
/// `identity.ts`. An unknown type gets a plain trim, which is that
/// function's `default` branch.
fn normalise(key_type: &str, raw: &str) -> String {
    match key_type {
        "email" | "username" => trim_like_the_others(raw).to_lowercase(),
        // Everything that is not `+` or an ASCII digit goes, so
        // "+81 (90) 1234-5678" and "+819012345678" agree. Deliberately
        // not E.164 validation — that is the host's business; this
        // only removes formatting noise.
        "phone" => raw
            .chars()
            .filter(|c| *c == '+' || c.is_ascii_digit())
            .collect(),
        // OAuth subject claims are opaque. Touching them would change
        // an identifier the provider defined.
        "googleSub" | "appleSub" | "metaSub" => raw.to_string(),
        _ => trim_like_the_others(raw).to_string(),
    }
}

/// Lowercase hex SHA-256 of the normalised value. `None` for an empty
/// or whitespace-only input — no key beats a meaningless one, and the
/// caller treats absent as "not addressable".
#[must_use]
pub fn hash(key_type: &str, value: &str) -> Option<String> {
    let normalised = normalise(key_type, value);
    if normalised.is_empty() {
        return None;
    }
    Some(hex::encode(Sha256::digest(normalised.as_bytes())))
}

/// The key a device carries for this app user id.
///
/// `userKey(id, email)` in the SDKs takes the id when there is one
/// and the email otherwise; a caller targeting by `appUserId` is
/// giving the id, so that is the branch this is.
#[must_use]
pub fn user_key_for_app_user_id(app_user_id: &str) -> Option<String> {
    hash("id", app_user_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    /// Every vector the other three implementations are held to.
    ///
    /// Read from the file rather than copied into this test, so a
    /// regenerated fixture reaches this implementation too. A copy
    /// would drift on the first change and go on passing.
    #[test]
    fn this_agrees_with_the_other_three_implementations() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../sdk/native/fixtures/identity-vectors.json"
        );
        // An unreadable *or* empty file both mean the same thing here:
        // a gate that reads nothing passes everything.
        let raw = std::fs::read_to_string(path).unwrap_or_default();
        assert!(
            !raw.is_empty(),
            "identity vectors missing or empty at {path} — the thing holding \
             four implementations together is that file"
        );
        let doc: Value = serde_json::from_str(&raw).unwrap_or(Value::Null);
        let vectors = doc
            .get("vectors")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        assert!(
            vectors.len() >= 13,
            "read {} vectors — a checker that reads nothing passes on anything",
            vectors.len()
        );

        for v in &vectors {
            let key_type = v.get("keyType").and_then(Value::as_str).unwrap_or("");
            let input = v.get("raw").and_then(Value::as_str).unwrap_or("");
            let want = v.get("sha256Hex").and_then(Value::as_str);
            let got = hash(key_type, input);
            assert_eq!(
                got.as_deref(),
                want,
                "{key_type} {input:?}: this implementation disagrees with the one \
                 the devices use, so a device would stop matching its own events"
            );
        }
    }

    #[test]
    fn an_empty_id_has_no_key() {
        assert_eq!(hash("id", ""), None);
        assert_eq!(hash("id", "   "), None);
        assert_eq!(hash("id", "\u{feff} \u{00a0}"), None);
    }

    /// The branch a caller targeting by `appUserId` lands on.
    #[test]
    fn targeting_by_app_user_id_takes_the_id_branch() {
        assert_eq!(user_key_for_app_user_id("usr_123"), hash("id", "usr_123"));
        assert_eq!(user_key_for_app_user_id(""), None);
    }
}
