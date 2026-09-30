//! GET /admin/api/projects/:project_id/push/sends
//!
//! Recent push attempts for ops triage — failed retries, slow
//! sends, vendor-error pattern hunting.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Extension, Path, Query, State},
    http::StatusCode,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

use crate::session_mw::SessionContext;
use crate::state::AppState;

#[derive(Deserialize, Default)]
pub struct ListQuery {
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

pub async fn retry(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path((project_id, send_id)): Path<(Uuid, Uuid)>,
) -> (axum::http::StatusCode, Json<Value>) {
    use axum::http::StatusCode;
    if let Err(e) = super::tokens::ensure_project_access(&state, &ctx, project_id).await {
        return e;
    }
    // `AND project_id` ties the send to the guarded project so a
    // send_id from another project (even in this workspace) can't be
    // retried through the wrong project's URL.
    let res = sqlx::query(
        "UPDATE push_sends SET status = 'queued', next_attempt_at = now(), \
            retry_count = 0, error = NULL \
         WHERE id = $1 AND project_id = $2 AND status = 'failed' RETURNING id",
    )
    .bind(send_id)
    .bind(project_id)
    .fetch_optional(&state.pool)
    .await;
    match res {
        Ok(Some(_)) => {
            crate::audit::record(
                &state.pool,
                None,
                ctx.user_id,
                "push.retry",
                "push_send",
                &send_id.to_string(),
                json!({}),
            )
            .await;
            (
                StatusCode::ACCEPTED,
                Json(json!({ "send_id": send_id.to_string(), "status": "queued" })),
            )
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "not_failed_or_missing" })),
        ),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "internal" })),
        ),
    }
}

/// POST /admin/api/projects/:project_id/push/sends/_retry_all_failed
/// Unstuck the whole DLQ at once. Useful after fixing a bad
/// credential — all the rows that piled up while the cred was
/// wrong get one more chance.
pub async fn retry_all_failed(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
) -> (axum::http::StatusCode, Json<Value>) {
    // 403, like the eleven sibling routes on this project.
    // Answering `[]` made "you may not look" indistinguishable
    // from "there are none", which is the exact question a
    // setup screen is asking.
    if let Err(e) = super::tokens::ensure_project_access(&state, &ctx, project_id).await {
        return e;
    }
    let res = sqlx::query(
        "UPDATE push_sends SET status = 'queued', next_attempt_at = now(), \
            retry_count = 0, error = NULL \
         WHERE project_id = $1 AND status = 'failed' RETURNING id",
    )
    .bind(project_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();
    let count = res.len();
    crate::audit::record(
        &state.pool,
        Some(project_id),
        ctx.user_id,
        "push.retry_all_failed",
        "push_send",
        "",
        json!({ "count": count }),
    )
    .await;
    (
        axum::http::StatusCode::OK,
        Json(json!({ "requeued": count })),
    )
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
    Query(q): Query<ListQuery>,
) -> (axum::http::StatusCode, Json<Value>) {
    // 403, like the eleven sibling routes on this project.
    // Answering `[]` made "you may not look" indistinguishable
    // from "there are none", which is the exact question a
    // setup screen is asking.
    if let Err(e) = super::tokens::ensure_project_access(&state, &ctx, project_id).await {
        return e;
    }
    let limit = i64::from(q.limit.unwrap_or(50).clamp(1, 1000));
    let offset = i64::from(q.offset.unwrap_or(0));
    // One statement with a nullable filter, rather than two that had
    // to be kept identical by hand. They already differed by a
    // placeholder number, which is the kind of divergence that ends
    // with one branch selecting a column the other does not.
    let status = q.status.as_deref().filter(|s| !s.is_empty());

    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM push_sends \
         WHERE project_id = $1 AND ($2::text IS NULL OR status = $2)",
    )
    .bind(project_id)
    .bind(status)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    let rows = sqlx::query(
        "SELECT id, token_id, provider, status, provider_outcome, error, retry_count, \
                payload, created_at, sent_at, next_attempt_at \
         FROM push_sends \
         WHERE project_id = $1 AND ($2::text IS NULL OR status = $2) \
         ORDER BY created_at DESC LIMIT $3 OFFSET $4",
    )
    .bind(project_id)
    .bind(status)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();
    let out: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<Uuid, _>("id").to_string(),
                "token_id": r.get::<Uuid, _>("token_id").to_string(),
                "provider": r.get::<String, _>("provider"),
                "status": r.get::<String, _>("status"),
                "provider_outcome": r.try_get::<Option<String>, _>("provider_outcome").ok().flatten(),
                "error": r.try_get::<Option<String>, _>("error").ok().flatten(),
                "retry_count": r.try_get::<i32, _>("retry_count").unwrap_or(0),
                // What was actually sent. The table listed rows and
                // their outcomes and never said which notification
                // they were, so a failed row could not be matched to
                // the send that produced it — and nothing anywhere
                // could observe that a queued row carried the right
                // message at all.
                "payload": r.try_get::<Value, _>("payload").unwrap_or_else(|_| json!({})),
                "created_at": crate::wire_time::rfc3339(r.get::<time::OffsetDateTime, _>("created_at")),
                "sent_at": crate::wire_time::rfc3339_opt(r.try_get::<Option<time::OffsetDateTime>, _>("sent_at").ok().flatten()),
                "next_attempt_at": crate::wire_time::rfc3339_opt(r.try_get::<Option<time::OffsetDateTime>, _>("next_attempt_at").ok().flatten()),
            })
        })
        .collect();
    (
        axum::http::StatusCode::OK,
        Json(json!({ "sends": out, "total": total, "offset": offset })),
    )
}

/// `GET /admin/api/projects/:project_id/push/health`
///
/// One request behind the delivery card. The dashboard used to have
/// no way to see push at all; when it got one, aggregating a
/// thousand-row send list in the browser to answer "is delivery
/// working" would have been the wrong shape — the question is a
/// count, and counts belong in the database.
///
/// `reasons` is the point: "12 failed" is an alarm, "12 failed,
/// BadDeviceToken" is a fix.
pub async fn health(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    super::tokens::ensure_project_access(&state, &ctx, project_id).await?;

    let row = sqlx::query(
        "SELECT \
           count(*) FILTER (WHERE status = 'sent'   AND created_at > now() - interval '24 hours') AS sent24h, \
           count(*) FILTER (WHERE status = 'failed' AND created_at > now() - interval '24 hours') AS failed24h, \
           count(*) FILTER (WHERE status = 'queued') AS queued, \
           max(created_at) AS last_send_at \
         FROM push_sends WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| internal(&e))?;

    let reasons = sqlx::query(
        "SELECT coalesce(nullif(error, ''), provider_outcome, 'unknown') AS reason, count(*) AS n \
         FROM push_sends \
         WHERE project_id = $1 AND status = 'failed' \
           AND created_at > now() - interval '24 hours' \
         GROUP BY 1 ORDER BY n DESC LIMIT 8",
    )
    .bind(project_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| internal(&e))?;

    // `device_tokens` is the canonical store — send, subscribe and
    // preferences all query it, and it is the one carrying the
    // identity an issue can address.
    let tokens = sqlx::query(
        "SELECT count(*) FILTER (WHERE revoked_at IS NULL) AS live, \
                count(*) FILTER (WHERE revoked_at IS NOT NULL) AS quarantined, \
                count(*) FILTER (WHERE revoked_at IS NULL AND user_key IS NOT NULL) \
                  AS identified \
         FROM device_tokens WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| internal(&e))?;

    Ok(Json(json!({
        "sent24h": row.get::<i64, _>("sent24h"),
        "failed24h": row.get::<i64, _>("failed24h"),
        "queued": row.get::<i64, _>("queued"),
        "lastSendAt": crate::wire_time::rfc3339_opt(
            row.try_get::<Option<time::OffsetDateTime>, _>("last_send_at").ok().flatten()
        ),
        "liveTokens": tokens.get::<i64, _>("live"),
        "quarantinedTokens": tokens.get::<i64, _>("quarantined"),
        // Devices a specific user's issue can reach. The rest can
        // only be broadcast to.
        "identifiedTokens": tokens.get::<i64, _>("identified"),
        "reasons": reasons.iter().map(|r| json!({
            "reason": r.get::<String, _>("reason"),
            "count": r.get::<i64, _>("n"),
        })).collect::<Vec<_>>(),
    })))
}

fn internal(e: &sqlx::Error) -> (StatusCode, Json<Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": e.to_string() })),
    )
}
