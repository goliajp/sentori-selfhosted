//! Per-user session list + targeted revoke for the Settings page.
//!
//! Resolves current user via SessionContext; list+revoke scoped
//! to that user so an attacker can't enumerate other users'
//! session ids.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Extension, Path, State},
    http::{HeaderMap, StatusCode},
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
        "SELECT id_hash_hex, created_at, last_used_at, expires_at, ip, user_agent \
         FROM sessions WHERE user_id = $1 ORDER BY last_used_at DESC NULLS LAST",
    )
    .bind(ctx.user_id.into_uuid())
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();
    let out: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id_hash_hex": r.get::<String, _>("id_hash_hex"),
                "created_at": crate::wire_time::rfc3339(r.get::<time::OffsetDateTime, _>("created_at")),
                "last_used_at": crate::wire_time::rfc3339_opt(r.try_get::<Option<time::OffsetDateTime>, _>("last_used_at").ok().flatten()),
                "expires_at": crate::wire_time::rfc3339(r.get::<time::OffsetDateTime, _>("expires_at")),
                "ip": r.try_get::<Option<String>, _>("ip").ok().flatten(),
                "user_agent": r.try_get::<Option<String>, _>("user_agent").ok().flatten(),
            })
        })
        .collect();
    Json(json!({ "sessions": out }))
}

pub async fn revoke(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(id_hash_hex): Path<String>,
    headers: HeaderMap,
) -> StatusCode {
    let res = sqlx::query("DELETE FROM sessions WHERE user_id = $1 AND id_hash_hex = $2")
        .bind(ctx.user_id.into_uuid())
        .bind(&id_hash_hex)
        .execute(&state.pool)
        .await;
    match res {
        Ok(r) if r.rows_affected() > 0 => {
            let (ip, ua) = crate::notify::extract_request_meta(&headers);
            crate::notify::audit(
                &state.pool,
                ctx.workspace_id.into_uuid(),
                None,
                Some(ctx.user_id.into_uuid()),
                "session.revoke",
                Some("session"),
                Some(&id_hash_hex),
                crate::notify::enrich_payload(json!({}), ip.as_deref(), ua.as_deref()),
            )
            .await;
            StatusCode::NO_CONTENT
        }
        Ok(_) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
