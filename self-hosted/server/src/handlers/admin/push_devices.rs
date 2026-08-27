//! GET /admin/api/projects/:project_id/push/devices
//!
//! The devices registered against a project, with what each one
//! reported about itself.
//!
//! Until now the dashboard could say how many devices existed and how
//! many were addressable, and nothing else. An integrator who passed
//! `metadata` to `register()` had no way to confirm it arrived — and
//! it did not, because neither the SDK nor the register handler ever
//! carried it. "If the device row could show its metadata we could
//! check this ourselves instead of asking you" is exactly the right
//! ask (insight, 2026-08-11), and it is also how a silently-dropped
//! field gets noticed the next time.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Extension, Path, Query, State},
    http::StatusCode,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use tracing::warn;
use uuid::Uuid;

use crate::session_mw::SessionContext;
use crate::state::AppState;

#[derive(Deserialize, Default)]
pub struct ListQuery {
    /// `live` (default) hides revoked rows; `all` shows them.
    pub scope: Option<String>,
    pub limit: Option<u32>,
    /// How many to skip. There was no way to skip any, so a project
    /// with more devices than the cap had rows nobody could reach —
    /// the console showed the first hundred and gave no sign there
    /// were others.
    pub offset: Option<u32>,
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    super::tokens::ensure_project_access(&state, &ctx, project_id).await?;

    let include_revoked = q.scope.as_deref() == Some("all");
    let limit = i64::from(q.limit.unwrap_or(50).min(500));
    let offset = i64::from(q.offset.unwrap_or(0));

    // The count comes back with the page. A table that says "50" when
    // there are four hundred is not a smaller truth, it is a
    // different one, and the number is what tells a reader whether
    // what they are looking at is all of it.
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM device_tokens \
         WHERE project_id = $1 AND ($2 OR revoked_at IS NULL)",
    )
    .bind(project_id)
    .bind(include_revoked)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    let rows = sqlx::query(
        "SELECT id, provider, env, metadata, traits, bad_streak, revoked_at, \
                last_seen_at, created_at, \
                user_key IS NOT NULL AS addressable, \
                right(user_key, 6) AS user_key_tail, \
                right(native_token, 6) AS token_tail \
         FROM device_tokens \
         WHERE project_id = $1 AND ($2 OR revoked_at IS NULL) \
         ORDER BY last_seen_at DESC LIMIT $3 OFFSET $4",
    )
    .bind(project_id)
    .bind(include_revoked)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| {
        warn!(error = %e, "admin.push.devices query failed");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "internal" })),
        )
    })?;

    let devices: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<Uuid, _>("id").to_string(),
                "provider": r.get::<String, _>("provider"),
                "env": r.get::<Option<String>, _>("env"),
                // The metadata the host sent, verbatim. Whether it is
                // `{}` is the answer to "did my metadata arrive".
                "metadata": r.get::<Value, _>("metadata"),
                // What the host said about the *person*. Selectable by
                // a send, unlike the identity, which is only ever a
                // hash — so this is the column that answers "will my
                // campaign's condition match this device", and there
                // was nowhere to check that before sending.
                "traits": r.get::<Value, _>("traits"),
                // Whether `sentori.user()` ran before `register()`.
                // The raw key is never returned — this row exists to
                // answer "can this device be reached", not to hand
                // back identity material.
                // Both, on purpose. The boolean is what the send
                // path cares about; the tail is what a person needs
                // to believe it. A column reading only "addressable /
                // not" has now sent two readers to the source to find
                // out what it meant — it asks whether `sentori.user()`
                // ran before `register()`, which is not what the word
                // suggests. Showing the key itself needs no word.
                "addressable": r.get::<bool, _>("addressable"),
                "userKeyTail": r.get::<Option<String>, _>("user_key_tail"),
                "badStreak": r.get::<i32, _>("bad_streak"),
                "revokedAt": r.get::<Option<time::OffsetDateTime>, _>("revoked_at")
                    .map(crate::wire_time::rfc3339),
                "lastSeenAt": crate::wire_time::rfc3339(r.get::<time::OffsetDateTime, _>("last_seen_at")),
                "createdAt": crate::wire_time::rfc3339(r.get::<time::OffsetDateTime, _>("created_at")),
                // Enough to tell two devices apart in the UI, not
                // enough to send to one. A push token is a capability.
                "tokenTail": r.get::<Option<String>, _>("token_tail"),
            })
        })
        .collect();

    Ok(Json(
        json!({ "devices": devices, "total": total, "offset": offset }),
    ))
}

/// Retire a device from the console.
///
/// The SDK can revoke its own registration, and quarantine retires a
/// token a provider has declared dead — but an operator looking at a
/// device that should stop receiving had nowhere to click. The only
/// routes to `revoked_at` needed either the app or a failed delivery.
///
/// Sets the same column the send path filters on and quarantine
/// writes, so a device retired here is retired the same way as one
/// retired any other way. A later `register` from the same device
/// revives it, which is the documented behaviour and the reason this
/// is a revocation rather than a delete: the row keeps its history.
pub async fn revoke(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path((project_id, token_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    super::tokens::ensure_project_access(&state, &ctx, project_id).await?;

    // `AND project_id` ties the token to the guarded project, so an
    // id from another project cannot be retired through this one's
    // URL.
    let affected = sqlx::query(
        "UPDATE device_tokens SET revoked_at = now() \
         WHERE project_id = $1 AND id = $2 AND revoked_at IS NULL",
    )
    .bind(project_id)
    .bind(token_id)
    .execute(&state.pool)
    .await
    .map_err(|e| {
        warn!(error = %e, "admin.push.devices revoke failed");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "internal" })),
        )
    })?
    .rows_affected();

    // Says which. An id that matched nothing — already revoked, or
    // another project's — answering exactly like a revocation is how
    // the SDK's own endpoint hid a bug for a year.
    Ok(Json(json!({
        "status": if affected > 0 { "revoked" } else { "not_found" },
    })))
}
