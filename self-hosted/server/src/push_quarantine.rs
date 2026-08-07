//! Per-token quarantine handler.
//!
//! When a vendor returns a "permanent token failure" code (APNs
//! BadDeviceToken, FCM NotRegistered, MiPush invalid regid),
//! mark the device_tokens row revoked so subsequent sends skip
//! it. Transient failures (5xx, 429) leave the token alone but
//! bump a streak counter for L1 backoff; >3 strikes also
//! quarantines the token so a chronically-bad token doesn't
//! waste retry budget.

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

/// Decide which response code is a permanent token failure that
/// should quarantine vs a transient failure that should just bump
/// the streak.
// Arms are deliberately kept per-provider rather than merged: each
// carries the vendor-specific reasoning for why that status is
// treated as permanent, which a combined pattern would lose.
#[allow(clippy::match_same_arms)]
#[must_use]
pub fn is_permanent_token_failure(provider: &str, http_status: u16) -> bool {
    match (provider, http_status) {
        // APNs: 400 BadDeviceToken / 410 Unregistered / 410 BadCollapseId
        ("apns", 400 | 410) => true,
        // FCM legacy: 200 with results.error == NotRegistered → caller
        // would need to inspect body; we conservatively quarantine on
        // 404 here.
        ("fcm", 404) => true,
        // WebPush: 404 Subscription Not Found / 410 Subscription Gone
        ("webpush", 404 | 410) => true,
        // HCM: per docs, 404 = invalid token
        ("hcm", 404) => true,
        // MiPush: 200 + error in body; we look at HTTP only here
        ("mipush", 400 | 404) => true,
        _ => false,
    }
}
