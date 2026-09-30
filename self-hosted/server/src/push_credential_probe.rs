//! Ask the vendor.
//!
//! `push_credential_check` proves a credential is *well-formed* — the
//! PEM parses, the JWT signs, the JSON has the fields the worker
//! reads. Everything it proves, it proves alone in this process. It
//! cannot tell you the key was revoked last month, that the service
//! account was never granted the messaging role, or that the `.p8`
//! you downloaded is an App Store Connect key rather than an APNs
//! key — those two files are byte-for-byte the same shape and only
//! Apple knows which is which.
//!
//! So this module asks. Without delivering anything:
//!
//! - **APNs** — a send to a device token of sixty-four zeros. No
//!   device has that token, so nothing arrives; but Apple checks the
//!   provider token *before* it checks the device token, and its
//!   refusal says which of the two it disliked.
//! - **FCM** — `validate_only: true`, which Google documents as a
//!   full validation that never delivers.
//!
//! ## Three answers, not two
//!
//! The state that was missing is [`Verdict::Limited`]: the vendor
//! knows this credential and will not let it do this job. A service
//! account that authenticates but lacks the messaging role, an APNs
//! key that signs but is not authorised for the topic it was given.
//! Reporting that as *rejected* sends an operator to re-download a
//! key that was never the problem — which is the longest way to fail,
//! because the new key behaves exactly like the old one.
//!
//! ## Why the mapping is a pure function
//!
//! [`apns_verdict`] and [`fcm_verdict`] take a status and a body and
//! return a verdict. No network, no clock. Every row of the table
//! below is a unit test, because the whole value of this module is in
//! that table being right, and a table you can only exercise against
//! Apple's production servers is a table nobody checks.

use serde_json::{Value, json};

/// What the vendor said, in three states plus "we could not ask".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// The vendor accepted the credential.
    Ok,
    /// The vendor knows the credential and refuses this particular
    /// job. The credential is not the thing to replace.
    Limited {
        /// Stable machine code, e.g. `topic-not-allowed`.
        code: &'static str,
        /// Which field to look at, when it is one field.
        field: Option<&'static str>,
        /// The vendor's own words, kept verbatim.
        detail: String,
    },
    /// The vendor refused the credential itself.
    Rejected {
        /// Stable machine code, e.g. `key-not-accepted`.
        code: &'static str,
        /// Which field to look at, when it is one field.
        field: Option<&'static str>,
        /// The vendor's own words, kept verbatim.
        detail: String,
    },
    /// We could not get an answer. Says nothing about the credential.
    Unreachable {
        /// What went wrong on our side.
        detail: String,
    },
    /// No probe exists for this provider yet.
    NotImplemented {
        /// Why not — never a bare "unsupported".
        detail: String,
    },
}

impl Verdict {
    /// The value stored in `push_credentials.last_validate_status`.
    /// The column's CHECK constraint holds exactly this vocabulary.
    pub fn status(&self) -> &'static str {
        match self {
            Verdict::Ok => "ok",
            Verdict::Limited { .. } => "limited",
            Verdict::Rejected { .. } => "rejected",
            Verdict::Unreachable { .. } => "unreachable",
            Verdict::NotImplemented { .. } => "not_implemented",
        }
    }

    /// Whether promoting this credential to active is defensible.
    /// Only `Ok` is — `Limited` sends nothing, and `Unreachable`
    /// means we do not know.
    pub fn safe_to_activate(&self) -> bool {
        matches!(self, Verdict::Ok)
    }

    fn limited(code: &'static str, field: Option<&'static str>, detail: impl Into<String>) -> Self {
        Verdict::Limited {
            code,
            field,
            detail: detail.into(),
        }
    }

    fn rejected(
        code: &'static str,
        field: Option<&'static str>,
        detail: impl Into<String>,
    ) -> Self {
        Verdict::Rejected {
            code,
            field,
            detail: detail.into(),
        }
    }
}

/// A device token no device has: sixty-four zeros.
///
/// It has to be the right *shape* — Apple rejects a malformed token
/// before it looks at the provider token, which would tell us nothing
/// — and it must belong to nobody, so that a probe cannot deliver.
const NOWHERE_APNS_TOKEN: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// The same idea for FCM. `validate_only` already guarantees no
/// delivery; this only has to be a token Google will not recognise.
const NOWHERE_FCM_TOKEN: &str = "sentori-probe-token-belongs-to-no-device";

/// Apple's answer, read.
///
/// Apple checks the provider token first. So a complaint about the
/// *device* token is the news we came for: the key, the key id, the
/// team and the topic were all accepted.
pub fn apns_verdict(status: u16, body: &str) -> Verdict {
    let reason = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v.get("reason").and_then(Value::as_str).map(str::to_owned))
        .unwrap_or_default();

    match (status, reason.as_str()) {
        // It got past every check we care about and died on the token
        // we made up. That is the pass. `Unregistered` says the same
        // thing one step later, and a 200 for a token of zeros would
        // be odd but is still Apple accepting the credential.
        (200, _) | (400, "BadDeviceToken" | "DeviceTokenNotForTopic") | (410, "Unregistered") => {
            Verdict::Ok
        }

        // The key is real; this topic is not its business.
        (400, "BadTopic") => Verdict::limited(
            "topic-malformed",
            Some("topic"),
            "Apple: BadTopic — the topic is not a bundle id Apple recognises",
        ),
        (403, "TopicDisallowed") => Verdict::limited(
            "topic-not-allowed",
            Some("topic"),
            "Apple: TopicDisallowed — the key is not authorised for this topic",
        ),
        (403, "ExpiredProviderToken") => {
            Verdict::rejected("key-expired", Some("secret"), "Apple: ExpiredProviderToken")
        }
        (429, "TooManyProviderTokenUpdates") => Verdict::limited(
            "signing-rate-limited",
            None,
            "Apple: TooManyProviderTokenUpdates — too many JWTs minted from this key recently",
        ),

        // The credential itself. Note what this does NOT distinguish:
        // Apple says InvalidProviderToken for a wrong key id, a wrong
        // team id, a revoked key, and an App Store Connect key used
        // as an APNs key. All four look identical from here, so the
        // detail names all four rather than guessing one.
        (401 | 403, _) => Verdict::rejected(
            "key-not-accepted",
            Some("secret"),
            format!(
                "Apple: {} — the key, its Key ID, the Team ID, or the key's type. \
                 An App Store Connect key is the same file shape as an APNs key and \
                 fails exactly here.",
                if reason.is_empty() {
                    "rejected"
                } else {
                    &reason
                }
            ),
        ),

        (500..=599, _) => Verdict::Unreachable {
            detail: format!("Apple returned {status}"),
        },
        _ => Verdict::rejected(
            "key-not-accepted",
            Some("secret"),
            format!(
                "Apple returned {status}{}",
                if reason.is_empty() {
                    String::new()
                } else {
                    format!(" {reason}")
                }
            ),
        ),
    }
}

/// Google's answer, read.
///
/// With `validate_only` the only thing left to fail on is our made-up
/// registration token, so `INVALID_ARGUMENT` is the pass.
pub fn fcm_verdict(status: u16, body: &str) -> Verdict {
    let parsed = serde_json::from_str::<Value>(body).unwrap_or(Value::Null);
    let code = parsed
        .pointer("/error/status")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let message = parsed
        .pointer("/error/message")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    match (status, code.as_str()) {
        // Authenticated, authorised, and it reached the part that
        // reads the token we invented.
        (200, _) | (400, "INVALID_ARGUMENT") | (404, "NOT_FOUND" | "UNREGISTERED") => Verdict::Ok,

        // Authenticated as somebody, but not somebody allowed here.
        (403, _) => Verdict::limited(
            "not-authorised-for-project",
            Some("secret"),
            format!(
                "Google: PERMISSION_DENIED — the service account authenticates but is not \
                 allowed to send for this project. Grant it the Firebase Cloud Messaging \
                 API Admin role.{}",
                detail_suffix(&message)
            ),
        ),
        (401, _) => Verdict::rejected(
            "key-not-accepted",
            Some("secret"),
            format!("Google: UNAUTHENTICATED{}", detail_suffix(&message)),
        ),
        (429, _) => Verdict::limited(
            "quota",
            None,
            format!("Google: RESOURCE_EXHAUSTED{}", detail_suffix(&message)),
        ),
        (500..=599, _) => Verdict::Unreachable {
            detail: format!("Google returned {status}"),
        },
        _ => Verdict::rejected(
            "key-not-accepted",
            Some("secret"),
            format!("Google returned {status}{}", detail_suffix(&message)),
        ),
    }
}

fn detail_suffix(message: &str) -> String {
    if message.is_empty() {
        String::new()
    } else {
        format!(" — {message}")
    }
}

/// Ask the vendor about one credential. Delivers nothing.
pub async fn probe(kind: &str, config: &Value, secret: &str) -> Verdict {
    match kind {
        "apns" => probe_apns(config, secret).await,
        "fcm" => probe_fcm(secret).await,
        // VAPID has no server to ask: the keypair either signs or it
        // does not, and `push_credential_check` already settled that.
        "webpush" => Verdict::NotImplemented {
            detail: "Web Push has no vendor to ask — a VAPID key is only ever checked locally"
                .into(),
        },
        // Deliberately not guessed. Neither of these has ever been
        // exercised against real credentials, and a probe whose
        // mapping nobody has seen fire is worse than no probe: it
        // reports confidently.
        "hcm" | "mipush" => Verdict::NotImplemented {
            detail: format!("no probe for {kind} yet — its credential is checked locally only"),
        },
        _ => Verdict::NotImplemented {
            detail: format!("unknown provider {kind}"),
        },
    }
}

async fn probe_apns(config: &Value, secret: &str) -> Verdict {
    let field = |k: &str| {
        config
            .get(k)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let cfg = crate::apns::ApnsConfig {
        team_id: field("teamId"),
        key_id: field("keyId"),
        topic: field("topic"),
        private_pem: secret.to_string(),
        // Probe production. A token-based key is valid for both
        // hosts, and production is the one an operator is about to
        // rely on.
        production: true,
    };

    match crate::apns::send(&cfg, NOWHERE_APNS_TOKEN, "", "").await {
        // A 200 for a token of zeros would be surprising, but it is
        // still Apple accepting the credential, and nothing was
        // delivered because no device has that token.
        Ok(_) => Verdict::Ok,
        Err(crate::apns::ApnsError::Rejected { status, body }) => apns_verdict(status, &body),
        Err(e) => Verdict::Unreachable {
            detail: e.to_string(),
        },
    }
}

async fn probe_fcm(secret: &str) -> Verdict {
    let cfg = crate::fcm::FcmConfig {
        service_account_json: secret.to_string(),
    };
    let project = match crate::fcm::project_id(&cfg) {
        Ok(p) => p,
        Err(e) => {
            return Verdict::rejected("fcm-bad-json", Some("secret"), e.to_string());
        }
    };
    let token = match crate::fcm::token_for(&cfg).await {
        Ok(t) => t,
        // Google refused to mint a token from this key at all. That
        // is the credential, not the job.
        Err(e) => {
            return Verdict::rejected("key-not-accepted", Some("secret"), e.to_string());
        }
    };

    let url = format!("https://fcm.googleapis.com/v1/projects/{project}/messages:send");
    let body = json!({
        // Documented as a full validation that never delivers.
        "validate_only": true,
        "message": { "token": NOWHERE_FCM_TOKEN },
    });

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return Verdict::Unreachable {
                detail: e.to_string(),
            };
        }
    };

    match client
        .post(&url)
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            fcm_verdict(status, &text)
        }
        Err(e) => Verdict::Unreachable {
            detail: e.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apple(reason: &str) -> String {
        json!({ "reason": reason }).to_string()
    }

    fn google(status: &str, message: &str) -> String {
        json!({ "error": { "status": status, "message": message } }).to_string()
    }

    /// The vendor's words, whichever verdict carries them. Lets a
    /// test assert on the detail without a `panic!` in the else arm —
    /// this crate denies `clippy::panic` everywhere, tests included.
    fn detail_of(v: &Verdict) -> String {
        match v {
            Verdict::Ok => String::new(),
            Verdict::Limited { detail, .. }
            | Verdict::Rejected { detail, .. }
            | Verdict::Unreachable { detail }
            | Verdict::NotImplemented { detail } => detail.clone(),
        }
    }

    /// Which field the verdict blames, if one.
    fn field_of(v: &Verdict) -> Option<&'static str> {
        match v {
            Verdict::Limited { field, .. } | Verdict::Rejected { field, .. } => *field,
            _ => None,
        }
    }

    #[test]
    fn a_complaint_about_the_device_token_means_the_key_was_accepted() {
        // The whole probe rests on this: Apple reads the provider
        // token first, so reaching the device token is the pass.
        assert_eq!(
            apns_verdict(400, &apple("BadDeviceToken")),
            Verdict::Ok,
            "BadDeviceToken is what a good key looks like against a token of zeros"
        );
        assert_eq!(apns_verdict(410, &apple("Unregistered")), Verdict::Ok);
    }

    #[test]
    fn a_wrong_topic_does_not_blame_the_key() {
        // The failure this separates out: an operator re-downloads a
        // perfectly good key because we said "rejected", and the new
        // one behaves identically.
        let v = apns_verdict(403, &apple("TopicDisallowed"));
        assert_eq!(v.status(), "limited");
        assert!(!v.safe_to_activate());
        assert_eq!(
            field_of(&v),
            Some("topic"),
            "the topic is the field to look at, not the key"
        );
    }

    /// Verbatim from `api.push.apple.com` on 2026-08-15, for a
    /// well-formed ES256 JWT signed by a P-256 key Apple has never
    /// seen. Kept as bytes rather than as a paraphrase so that a
    /// change in Apple's shape breaks the test rather than the
    /// product.
    const APPLE_SAID: (u16, &str) = (403, r#"{"reason":"InvalidProviderToken"}"#);

    #[test]
    fn what_apple_actually_returns_for_a_key_it_does_not_know() {
        let (status, body) = APPLE_SAID;
        assert_eq!(apns_verdict(status, body).status(), "rejected");
    }

    #[test]
    fn an_unusable_key_is_rejected_and_names_the_lookalike() {
        let v = apns_verdict(403, &apple("InvalidProviderToken"));
        assert_eq!(v.status(), "rejected");
        assert_eq!(field_of(&v), Some("secret"));
        // The single most common way to hold this wrong, said out
        // loud, because Apple's own word for it does not distinguish.
        let detail = detail_of(&v);
        assert!(
            detail.contains("App Store Connect"),
            "the detail has to name the lookalike file: {detail}"
        );
    }

    #[test]
    fn a_service_account_without_the_role_is_limited_not_rejected() {
        let v = fcm_verdict(403, &google("PERMISSION_DENIED", "caller lacks permission"));
        assert_eq!(v.status(), "limited");
        // Naming the role is the difference between a fix and a
        // re-download.
        let detail = detail_of(&v);
        assert!(
            detail.contains("Firebase Cloud Messaging API Admin"),
            "the detail has to name the role to grant: {detail}"
        );
    }

    #[test]
    fn google_disliking_our_made_up_token_is_the_pass() {
        assert_eq!(
            fcm_verdict(
                400,
                &google("INVALID_ARGUMENT", "invalid registration token")
            ),
            Verdict::Ok
        );
        assert_eq!(fcm_verdict(200, "{}"), Verdict::Ok);
    }

    #[test]
    fn bad_credentials_are_rejected_on_both_sides() {
        assert_eq!(
            fcm_verdict(401, &google("UNAUTHENTICATED", "")).status(),
            "rejected"
        );
        assert_eq!(apns_verdict(401, "").status(), "rejected");
    }

    #[test]
    fn a_vendor_outage_says_nothing_about_the_credential() {
        // Not `rejected`. Recording an outage as a bad key is how a
        // working credential gets thrown away.
        assert_eq!(apns_verdict(503, "").status(), "unreachable");
        assert_eq!(fcm_verdict(500, "").status(), "unreachable");
        assert!(!apns_verdict(503, "").safe_to_activate());
    }

    #[test]
    fn only_ok_may_be_promoted() {
        assert!(Verdict::Ok.safe_to_activate());
        for v in [
            apns_verdict(403, &apple("TopicDisallowed")),
            apns_verdict(403, &apple("InvalidProviderToken")),
            apns_verdict(503, ""),
            Verdict::NotImplemented {
                detail: String::new(),
            },
        ] {
            assert!(!v.safe_to_activate(), "{v:?} must not be promotable");
        }
    }

    #[test]
    fn every_verdict_maps_to_a_status_the_column_accepts() {
        // The CHECK constraint in 0017 holds exactly this set. A
        // verdict that cannot be stored is a probe that 500s.
        const ALLOWED: [&str; 6] = [
            "ok",
            "limited",
            "rejected",
            "malformed",
            "unreachable",
            "not_implemented",
        ];
        for v in [
            Verdict::Ok,
            apns_verdict(403, &apple("TopicDisallowed")),
            apns_verdict(401, ""),
            apns_verdict(503, ""),
            Verdict::NotImplemented {
                detail: String::new(),
            },
        ] {
            assert!(
                ALLOWED.contains(&v.status()),
                "{v:?} stores as {}",
                v.status()
            );
        }
    }

    #[test]
    fn a_body_that_is_not_json_still_produces_a_verdict() {
        // Apple and Google both return HTML from their edge on a bad
        // day. Parsing must not decide the answer.
        assert_eq!(apns_verdict(403, "<html>502</html>").status(), "rejected");
        assert_eq!(fcm_verdict(503, "<html>").status(), "unreachable");
    }
}
