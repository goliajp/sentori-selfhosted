//! Per-token quarantine handler.
//!
//! When a vendor returns a "permanent token failure" code (APNs
//! BadDeviceToken, FCM NotRegistered, MiPush invalid regid),
//! mark the device_tokens row revoked so subsequent sends skip
//! it. Transient failures (5xx, 429) leave the token alone and bump a
//! streak on the device.
//!
//! The streak drives nothing, and that is deliberate. This comment
//! used to say it fed the backoff and that four strikes quarantined
//! the token; neither was true — the backoff is computed from
//! `push_sends.retry_count`, and nothing read the streak at all.
//!
//! It should stay that way. A transient failure is usually the
//! vendor's, and the vendor fails for every device at once, so
//! "quarantine at four in a row" is the fleet-wide rule this module
//! warns about below wearing a per-device disguise: one bad hour at
//! APNs and every device in the project is retired, each row keeping a
//! reason that was never about it.
//!
//! What the streak is for is a person: "this one device has failed
//! seven times running" is worth seeing next to it in the console, and
//! that is where it is shown.

use sqlx::PgPool;
use uuid::Uuid;

pub async fn quarantine_token(pool: &PgPool, token_id: Uuid, reason: &str) {
    let _ = sqlx::query(
        "UPDATE device_tokens SET revoked_at = now(), \
            metadata = metadata || jsonb_build_object('quarantine_reason', $2::text) \
         WHERE id = $1",
    )
    .bind(token_id)
    .bind(reason)
    .execute(pool)
    .await;
}

pub async fn bump_streak(pool: &PgPool, token_id: Uuid) {
    let _ = sqlx::query("UPDATE device_tokens SET bad_streak = bad_streak + 1 WHERE id = $1")
        .bind(token_id)
        .execute(pool)
        .await;
}

pub async fn reset_streak(pool: &PgPool, token_id: Uuid) {
    let _ = sqlx::query("UPDATE device_tokens SET bad_streak = 0 WHERE id = $1 AND bad_streak > 0")
        .bind(token_id)
        .execute(pool)
        .await;
}

/// Is this failure about *this device's token*, or about the way the
/// send was set up?
///
/// The difference decides whether one device is retired or nothing
/// is. Getting it wrong in the generous direction is expensive:
/// every device in a project fails the same way on a misconfigured
/// credential, so a rule that reads "any 404 means this token is
/// dead" retires the entire fleet on one operator's typo, and each
/// row keeps a reason that is a sentence about a token which was
/// never the problem.
///
/// The status alone cannot tell them apart. FCM's v1 body does —
/// insight's first real failure came back as
///
/// ```text
/// status=404 body={"error":{"code":404,"message":"NotRegistered",
///   "status":"NOT_FOUND","details":[{"errorCode":"UNREGISTERED"}]}}
/// ```
///
/// and APNs puts a `reason` in its 400s, most of which are about the
/// request rather than the device: `BadTopic`, `TopicDisallowed`,
/// `BadExpirationDate` describe a configuration that is wrong for
/// every device at once.
///
/// So: quarantine when the provider names the token, and not when it
/// merely answers with a status that *can* mean that. Anything
/// unrecognised is treated as not-the-token, because the cost of
/// being wrong runs the other way — a device that should have been
/// retired is retired on the next attempt, whereas a fleet retired
/// by mistake comes back only when every one of them registers
/// again.
#[must_use]
pub fn is_permanent_token_failure(provider: &str, http_status: u16, reason: &str) -> bool {
    match provider {
        // 410 Unregistered is unambiguous. A 400 is not: it is the
        // status for most of what can be wrong with a request, and
        // only two of its reasons are about the device.
        "apns" => {
            http_status == 410
                || (http_status == 400
                    && (reason.contains("BadDeviceToken")
                        || reason.contains("DeviceTokenNotForTopic")
                        || reason.contains("MissingDeviceToken")))
        }
        // v1 says which. `UNREGISTERED` is the token; a 404 without
        // it is something else — the project in the URL, most
        // likely, which is true of every device at once.
        "fcm" => {
            http_status == 404
                && (reason.contains("UNREGISTERED") || reason.contains("NotRegistered"))
        }
        // Web Push: 404 Subscription Not Found / 410 Subscription
        // Gone. Both name the subscription itself; there is no
        // project-level 404 on an endpoint that *is* the
        // subscription.
        "webpush" => matches!(http_status, 404 | 410),
        // HCM and MiPush report in the body too, but nothing here has
        // ever run against either, so there is no observed string to
        // match on. Keeping the old status-only rule would be a guess
        // dressed as a decision; not quarantining is the safe half of
        // the same guess, and the send still fails either way. They
        // fall through with everything else.
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The body insight actually received, from the first real
    /// provider rejection this server ever recorded.
    const FCM_DEAD_TOKEN: &str = r#"fcm rejected: status=404 body={ "error": { "code": 404, "message": "NotRegistered", "status": "NOT_FOUND", "details": [{ "errorCode": "UNREGISTERED" }] } }"#;

    #[test]
    fn a_provider_naming_the_token_retires_it() {
        assert!(is_permanent_token_failure("fcm", 404, FCM_DEAD_TOKEN));
        assert!(is_permanent_token_failure(
            "apns",
            410,
            "reason: Unregistered"
        ));
        assert!(is_permanent_token_failure(
            "apns",
            400,
            "reason: BadDeviceToken"
        ));
        assert!(is_permanent_token_failure("webpush", 410, ""));
    }

    /// The half that matters. Each of these fails identically for
    /// every device in the project, and retiring them all is a much
    /// worse day than retrying.
    #[test]
    fn a_failure_that_is_not_about_the_device_leaves_it_alone() {
        assert!(
            !is_permanent_token_failure(
                "fcm",
                404,
                "fcm rejected: status=404 body={\"error\":{\"status\":\"NOT_FOUND\",\"message\":\"Requested entity was not found.\"}}"
            ),
            "a 404 that does not name the token is the project in the URL, and \
             that is wrong for every device at once"
        );
        assert!(
            !is_permanent_token_failure("apns", 400, "reason: BadTopic"),
            "BadTopic describes the request, not the device"
        );
        assert!(!is_permanent_token_failure(
            "apns",
            400,
            "reason: BadExpirationDate"
        ));
        assert!(!is_permanent_token_failure("fcm", 401, "oauth: status=401"));
        assert!(!is_permanent_token_failure(
            "fcm",
            503,
            "backend unavailable"
        ));
    }
}
