//! Notification broadcast helpers — pushes a row into the
//! `notifications` table for every watcher of an issue when
//! an event (comment / status flip) happens.

use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

/// Extract requester IP + user-agent from request headers.
/// IP is honored from `X-Forwarded-For` (first hop) if present,
/// then `X-Real-IP`. UA from `User-Agent`. Both optional.
pub fn extract_request_meta(headers: &axum::http::HeaderMap) -> (Option<String>, Option<String>) {
    let ip = crate::client_ip::client_ip(headers);
    let ua = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.chars().take(200).collect::<String>())
        .filter(|s| !s.is_empty());
    (ip, ua)
}

/// Inject IP + user-agent into the audit payload before writing.
/// Pass `(None, None)` from background workers / non-request paths.
pub fn enrich_payload(mut payload: Value, ip: Option<&str>, user_agent: Option<&str>) -> Value {
    if ip.is_none() && user_agent.is_none() {
        return payload;
    }
    if let Some(map) = payload.as_object_mut() {
        if let Some(ip) = ip {
            map.insert("_ip".to_string(), Value::String(ip.to_string()));
        }
        if let Some(ua) = user_agent {
            map.insert("_ua".to_string(), Value::String(ua.to_string()));
        }
    }
    payload
}

/// Write an audit_log row. Best-effort; failure does not bubble
/// up — admin endpoint success is decoupled from the audit write.
// The parameters mirror the audit_log columns one-for-one; bundling
// them into a struct would add an indirection with no new invariant.
#[allow(clippy::too_many_arguments)]
pub async fn audit(
    pool: &PgPool,
    workspace_id: Uuid,
    project_id: Option<Uuid>,
    actor_user_id: Option<Uuid>,
    action: &str,
    target_type: Option<&str>,
    target_id: Option<&str>,
    payload: Value,
) {
    let _ = sqlx::query(
        "INSERT INTO audit_logs (id, workspace_id, project_id, actor_user_id, action, \
            target_type, target_id, payload) \
         VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(workspace_id)
    .bind(project_id)
    .bind(actor_user_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .bind(&payload)
    .execute(pool)
    .await;
}

/// Insert a notification per watcher (sans actor — they don't
/// need to notify themselves of their own action).
pub async fn notify_issue_watchers(
    pool: &PgPool,
    issue_id: Uuid,
    actor_user_id: Option<Uuid>,
    kind: &str,
    payload: Value,
) {
    let _ = sqlx::query(
        "INSERT INTO notifications (user_id, issue_id, kind, payload) \
         SELECT w.user_id, $3, $1, $2 \
         FROM watchers w \
         WHERE w.issue_id = $3 AND w.user_id IS DISTINCT FROM $4",
    )
    .bind(kind)
    .bind(&payload)
    .bind(issue_id)
    .bind(actor_user_id)
    .execute(pool)
    .await;
}
