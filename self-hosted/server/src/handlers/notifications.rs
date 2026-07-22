//! GET   /auth/notifications — current user's inbox
//! POST  /auth/notifications/:id/read — mark as read
//! POST  /auth/notifications/_read_all — mark all read

use std::sync::Arc;

use axum::{
    Json,
    extract::{Extension, Path, State},
    http::StatusCode,
};
use serde_json::{Value, json};
use sqlx::Row;

use crate::session_mw::SessionContext;
use crate::state::AppState;

pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
) -> Json<Value> {
    let rows = sqlx::query(
        "SELECT id, kind, payload, read_at, created_at FROM notifications \
         WHERE user_id = $1 ORDER BY created_at DESC LIMIT 100",
    )
    .bind(ctx.user_id.into_uuid())
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();
    let out: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<i64, _>("id").to_string(),
                "kind": r.get::<String, _>("kind"),
                "payload": r.try_get::<Value, _>("payload").unwrap_or(Value::Null),
                "read_at": crate::wire_time::rfc3339_opt(r.try_get::<Option<time::OffsetDateTime>, _>("read_at").ok().flatten()),
                "created_at": crate::wire_time::rfc3339(r.get::<time::OffsetDateTime, _>("created_at")),
            })
        })
        .collect();
    let unread = rows
        .iter()
        .filter(|r| {
            r.try_get::<Option<time::OffsetDateTime>, _>("read_at")
                .ok()
                .flatten()
                .is_none()
        })
        .count();
    Json(json!({ "notifications": out, "unread": unread }))
}

pub async fn read_one(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(notif_id): Path<i64>,
) -> StatusCode {
    let _ = sqlx::query(
        "UPDATE notifications SET read_at = now() \
         WHERE id = $1 AND user_id = $2 AND read_at IS NULL",
    )
    .bind(notif_id)
    .bind(ctx.user_id.into_uuid())
    .execute(&state.pool)
    .await;
    StatusCode::NO_CONTENT
}

pub async fn read_all(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
) -> StatusCode {
    let _ = sqlx::query(
        "UPDATE notifications SET read_at = now() WHERE user_id = $1 AND read_at IS NULL",
    )
    .bind(ctx.user_id.into_uuid())
    .execute(&state.pool)
    .await;
    StatusCode::NO_CONTENT
}
