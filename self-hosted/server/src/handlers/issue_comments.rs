//! Issue comments — markdown threaded discussion.
//!
//! - GET    /v1/issues/:issue_id/comments → list (read, public dashboard)
//! - POST   /admin/api/issues/:issue_id/comments → create (session-scoped)
//! - DELETE /admin/api/issues/:issue_id/comments/:comment_id (author only)

use std::sync::Arc;

use axum::{
    Json,
    extract::{Extension, Path, State},
    http::StatusCode,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

use crate::session_mw::SessionContext;
use crate::state::AppState;

pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(issue_id): Path<Uuid>,
) -> Result<Json<Value>, super::tenant::ApiErr> {
    super::tenant::guard_issue(&state, ctx.workspace_id, issue_id).await?;

    let rows = sqlx::query(
        // Join `users` for the same reason the members list does: a
        // comment is attributed to a person, and nobody recognises a
        // uuid as one.
        "SELECT c.id, c.author_id, c.body, c.created_at, u.email AS author_email \
         FROM issue_comments c \
         LEFT JOIN users u ON u.id = c.author_id \
         WHERE c.issue_id = $1 ORDER BY c.created_at",
    )
    .bind(issue_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();
    let out: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<Uuid, _>("id").to_string(),
                "author_user_id": r
                    .try_get::<Option<Uuid>, _>("author_id")
                    .ok()
                    .flatten()
                    .map(|u| u.to_string()),
                "author_email": r.try_get::<Option<String>, _>("author_email").ok().flatten(),
                "body_md": r.get::<String, _>("body"),
                "created_at": crate::wire_time::rfc3339(r.get::<time::OffsetDateTime, _>("created_at")),
                "edited_at": Option::<time::OffsetDateTime>::None,
            })
        })
        .collect();
    Ok(Json(json!({ "comments": out })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBody {
    pub body_md: String,
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(issue_id): Path<Uuid>,
    Json(body): Json<CreateBody>,
) -> (StatusCode, Json<Value>) {
    // Kept in this handler's `{ "error": ... }` body shape rather
    // than the guard's plain-text one.
    if let Err((status, msg)) = super::tenant::guard_issue(&state, ctx.workspace_id, issue_id).await
    {
        return (status, Json(json!({ "error": msg })));
    }
    if body.body_md.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "body_md required" })),
        );
    }
    let id = Uuid::now_v7();
    let res = sqlx::query(
        "INSERT INTO issue_comments (id, issue_id, author_id, body) \
         VALUES ($1, $2, $3, $4) RETURNING created_at",
    )
    .bind(id)
    .bind(issue_id)
    .bind(ctx.user_id.into_uuid())
    .bind(body.body_md.trim())
    .fetch_optional(&state.pool)
    .await;
    match res {
        Ok(Some(row)) => {
            // Fan out notifications to other watchers.
            crate::notify::notify_issue_watchers(
                &state.pool,
                issue_id,
                Some(ctx.user_id.into_uuid()),
                "comment",
                json!({
                    "issue_id": issue_id.to_string(),
                    "comment_id": id.to_string(),
                    "preview": body.body_md.trim().chars().take(80).collect::<String>(),
                }),
            )
            .await;
            (
                StatusCode::CREATED,
                Json(json!({
                    "id": id.to_string(),
                    "issue_id": issue_id.to_string(),
                    "author_user_id": ctx.user_id.into_uuid().to_string(),
                    // Same shape as the list response. Without it a
                    // freshly posted comment would show a uuid while
                    // every other comment showed an email, until reload.
                    "author_email": sqlx::query_scalar::<_, String>(
                        "SELECT email FROM users WHERE id = $1",
                    )
                    .bind(ctx.user_id.into_uuid())
                    .fetch_optional(&state.pool)
                    .await
                    .ok()
                    .flatten(),
                    "body_md": body.body_md.trim(),
                    "created_at": crate::wire_time::rfc3339(row.get::<time::OffsetDateTime, _>("created_at")),
                    // The list sends this; a create response that omits
                    // it is a second shape for the same object, and the
                    // client ends up describing both.
                    "edited_at": Value::Null,
                })),
            )
        }
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "internal" })),
        ),
    }
}

pub async fn delete(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path((issue_id, comment_id)): Path<(Uuid, Uuid)>,
) -> StatusCode {
    if let Err((status, _)) = super::tenant::guard_issue(&state, ctx.workspace_id, issue_id).await {
        return status;
    }
    // Only the author can delete their own comment.
    let res = sqlx::query("DELETE FROM issue_comments WHERE id = $1 AND author_id = $2")
        .bind(comment_id)
        .bind(ctx.user_id.into_uuid())
        .execute(&state.pool)
        .await;
    match res {
        Ok(r) if r.rows_affected() > 0 => StatusCode::NO_CONTENT,
        Ok(_) => StatusCode::FORBIDDEN,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
