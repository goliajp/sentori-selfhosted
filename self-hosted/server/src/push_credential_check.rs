//! What a provider credential has to look like, checked at the
//! moment it is pasted.
//!
//! Only FCM was checked. Everything else was stored verbatim and
//! failed later, on a device, as a message that never arrived — and
//! the two ways it failed were both invisible from the form:
//!
//! - The `.p8` went into a single-line `<input>`, and the HTML value
//!   sanitiser strips line breaks. A PEM with no line breaks is not a
//!   PEM, and `EncodingKey::from_ec_pem` says so — at delivery time,
//!   about a key nobody had touched since.
//! - The form's own placeholder read `bundleId`, and the worker reads
//!   `topic`. Following the example exactly produced
//!   `"topic missing"` on the first send.
//!
//! Both are the same defect: a credential is only known to be wrong
//! by trying to use it, and the trying happens somewhere the person
//! who pasted it is not looking. So the checks live here, and the
//! error names the field.

use serde_json::{Value, json};

/// Rejected, as something the console can say in its own language.
///
/// This carried English prose — a detail and a paragraph of guidance —
/// which the dashboard printed under a Chinese label. Half a sentence
/// in each language, in the one place an operator has already got
/// something wrong. Codes and a field name; the words live where they
/// are translated.
pub struct Rejection {
    /// What is wrong, as something to look up.
    pub code: &'static str,
    /// Which field, when the code is about one.
    pub field: Option<&'static str>,
    /// For the log, not for a person.
    pub detail: String,
}

impl Rejection {
    fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            field: None,
            detail: detail.into(),
        }
    }

    fn about(code: &'static str, field: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            field: Some(field),
            detail: detail.into(),
        }
    }
}

fn required<'a>(config: &'a Value, key: &str) -> Option<&'a str> {
    config
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
}

/// A PEM, and not the one-line remains of a PEM.
///
/// The header and footer survive an `<input>`; only the line breaks
/// between them do not. So a check that just looks for `BEGIN` would
/// pass exactly the paste this exists to catch.
fn looks_like_pem(secret: &str) -> Result<(), &'static str> {
    let trimmed = secret.trim();
    if !trimmed.starts_with("-----BEGIN") {
        return Err("pem-no-header");
    }
    if !trimmed.contains("-----END") {
        return Err("pem-no-footer");
    }
    if trimmed.lines().nth(1).is_none_or(|l| l.trim().is_empty()) {
        return Err("pem-one-line");
    }
    Ok(())
}

/// Check a credential and return the non-secret facts worth showing
/// back, merged into `config`.
///
/// Returning what was understood is half the point: an operator with
/// two Firebase projects or two Apple teams has no other way to see
/// which one they just pasted.
pub fn check(provider: &str, config: &mut Value, secret: &str) -> Result<(), Rejection> {
    match provider {
        "fcm" => {
            let cfg = crate::fcm::FcmConfig {
                service_account_json: secret.to_string(),
            };
            let project = crate::fcm::project_id(&cfg)
                .map_err(|e| Rejection::new("fcm-bad-json", e.to_string()))?;
            if let Some(map) = config.as_object_mut() {
                map.insert("fcmProjectId".into(), json!(project));
            }
            Ok(())
        }
        "apns" => {
            for key in ["keyId", "teamId", "topic"] {
                if required(config, key).is_none() {
                    return Err(Rejection::about(
                        "field-missing",
                        key,
                        format!("{key} is missing"),
                    ));
                }
            }
            looks_like_pem(secret)
                .map_err(|code| Rejection::about(code, "secret", "the pasted key is not a PEM"))?;
            crate::apns::mint_jwt(&crate::apns::ApnsConfig {
                team_id: required(config, "teamId").unwrap_or_default().to_string(),
                key_id: required(config, "keyId").unwrap_or_default().to_string(),
                topic: required(config, "topic").unwrap_or_default().to_string(),
                private_pem: secret.to_string(),
                production: true,
            })
            // The same signing the worker does, done now. A key that
            // cannot sign here will not sign tonight.
            .map_err(|e| Rejection::about("key-does-not-sign", "secret", e.to_string()))?;
            Ok(())
        }
        "webpush" => {
            if required(config, "vapidPublicKey").is_none() {
                return Err(Rejection::about(
                    "field-missing",
                    "vapidPublicKey",
                    "vapidPublicKey is missing",
                ));
            }
            looks_like_pem(secret)
                .map_err(|code| Rejection::about(code, "secret", "the pasted key is not a PEM"))?;
            Ok(())
        }
        // HCM and MiPush have never run against a real account here.
        // A check written from documentation alone would be a guess
        // that rejects valid credentials, which is worse than no
        // check: it makes a working setup impossible rather than
        // merely unverified.
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real EC key, so the APNs path is checked against something
    /// that actually signs rather than something that merely looks
    /// like a key.
    const EC_PEM: &str = "-----BEGIN PRIVATE KEY-----\nMIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgevZzL1gdAFr88hb2\nOF/2NxApJCzGCEDdfSp6VQO30hyhRANCAAQRWz+jn65BtOMvdyHKcvjBeBSDZH2r\n1RTwjmYSi9R/zpBnuQ4EiMnCqfMPWiZqB4QdbAd0E7oH50VpuZ1P087G\n-----END PRIVATE KEY-----";

    fn apns_config() -> Value {
        json!({ "keyId": "ABC1234567", "teamId": "DEF7654321", "topic": "com.example.app" })
    }

    #[test]
    fn a_key_flattened_by_a_single_line_field_is_refused() {
        // Exactly what the browser produces: same characters, no line
        // breaks. This is the paste the whole module was failing on.
        let flattened = EC_PEM.replace('\n', "");
        let mut config = apns_config();
        let rejection = check("apns", &mut config, &flattened).err();
        assert!(
            rejection.as_ref().is_some_and(|r| r.code == "pem-one-line"),
            "a PEM with no line breaks has to be refused, and the code has to \
             name the actual problem — got {:?}",
            rejection.map(|r| r.code)
        );
    }

    #[test]
    fn a_whole_key_is_accepted() {
        let mut config = apns_config();
        assert!(check("apns", &mut config, EC_PEM).is_ok());
    }

    /// The placeholder said `bundleId` and the worker reads `topic`.
    /// Following the example produced a credential that saved and
    /// then failed on the first send.
    #[test]
    fn the_field_the_worker_reads_is_the_field_that_is_required() {
        let mut config = json!({
            "keyId": "ABC1234567", "teamId": "DEF7654321", "bundleId": "com.example.app"
        });
        let rejection = check("apns", &mut config, EC_PEM).err();
        assert!(
            rejection
                .as_ref()
                .is_some_and(|r| r.code == "field-missing" && r.field == Some("topic")),
            "a credential naming bundleId rather than topic has to be refused — \
             got {:?}",
            rejection.map(|r| (r.code, r.field))
        );
    }

    #[test]
    fn fcm_still_reports_what_it_understood() {
        let mut config = json!({});
        let sa = r#"{"project_id":"qualcomm-insight","client_email":"a@b.iam.gserviceaccount.com","private_key":"-----BEGIN PRIVATE KEY-----\nMII\n-----END PRIVATE KEY-----\n"}"#;
        assert!(check("fcm", &mut config, sa).is_ok());
        assert_eq!(
            config.get("fcmProjectId").and_then(Value::as_str),
            Some("qualcomm-insight"),
            "an operator with two Firebase projects cannot otherwise see which \
             one they pasted"
        );
    }

    #[test]
    fn a_provider_with_no_observed_credential_is_not_guessed_at() {
        let mut config = json!({});
        assert!(check("hcm", &mut config, "whatever").is_ok());
    }
}
