//! GET /admin/api/projects/:project_id/push/sends
//!
//! Recent push attempts for ops triage — failed retries, slow
//! sends, vendor-error pattern hunting.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Extension, Path, Query, State},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

use crate::handlers::tenant::guard_project;
use crate::session_mw::SessionContext;
use crate::state::AppState;

#[derive(Deserialize, Default)]
pub struct ListQuery {
    pub status: Option<String>,
    pub limit: Option<u32>,
}

pub async fn retry(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path((project_id, send_id)): Path<(Uuid, Uuid)>,
) -> (axum::http::StatusCode, Json<Value>) {
    use axum::http::StatusCode;
    if let Err((code, msg)) = guard_project(&state, ctx.workspace_id, project_id).await {
        return (code, Json(json!({ "error": msg })));
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
            crate::notify::audit(
                &state.pool,
                ctx.workspace_id.into_uuid(),
                None,
                None,
                "push.retry",
                Some("push_send"),
                Some(&send_id.to_string()),
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
) -> Json<Value> {
    if guard_project(&state, ctx.workspace_id, project_id)
        .await
        .is_err()
    {
        return Json(json!({ "requeued": 0, "error": "not found" }));
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
    crate::notify::audit(
        &state.pool,
        ctx.workspace_id.into_uuid(),
        Some(project_id),
        None,
        "push.retry_all_failed",
        Some("push_send"),
        None,
        json!({ "count": count }),
    )
    .await;
    Json(json!({ "requeued": count }))
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
    Query(q): Query<ListQuery>,
) -> Json<Value> {
    if guard_project(&state, ctx.workspace_id, project_id)
        .await
        .is_err()
    {
        return Json(json!({ "sends": [] }));
    }
    let limit = i64::from(q.limit.unwrap_or(100).clamp(1, 1000));
    let rows = if let Some(status) = q.status.as_deref() {
        sqlx::query(
            "SELECT id, token_id, provider, status, provider_outcome, error, retry_count, \
                    created_at, sent_at, next_attempt_at \
             FROM push_sends \
             WHERE project_id = $1 AND status = $2 \
             ORDER BY created_at DESC LIMIT $3",
        )
        .bind(project_id)
        .bind(status)
        .bind(limit)
        .fetch_all(&state.pool)
        .await
    } else {
        sqlx::query(
            "SELECT id, token_id, provider, status, provider_outcome, error, retry_count, \
                    created_at, sent_at, next_attempt_at \
             FROM push_sends \
             WHERE project_id = $1 \
             ORDER BY created_at DESC LIMIT $2",
        )
        .bind(project_id)
        .bind(limit)
        .fetch_all(&state.pool)
        .await
    }
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
                "created_at": crate::wire_time::rfc3339(r.get::<time::OffsetDateTime, _>("created_at")),
                "sent_at": crate::wire_time::rfc3339_opt(r.try_get::<Option<time::OffsetDateTime>, _>("sent_at").ok().flatten()),
                "next_attempt_at": crate::wire_time::rfc3339_opt(r.try_get::<Option<time::OffsetDateTime>, _>("next_attempt_at").ok().flatten()),
            })
        })
        .collect();
    Json(json!({ "sends": out }))
}
