//! Dashboard auth: login / logout / me / change-password /
//! forgot-password / reset-password.
//!
//! No register, no verify, no OAuth (design.md §9): accounts are
//! created by the owner (or the env bootstrap), so the only public
//! entrances are login and the password-reset pair. Sessions are
//! opaque random tokens; the DB stores SHA-256 (see session_mw).
//!
//! Login answers with the same 401 for "no such user" and "wrong
//! password" — enumeration through error text is not on the menu.
//! The per-IP limiter on the bruteforce route group bounds the
//! guessing rate.

use std::sync::Arc;

use axum::{Extension, Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::Deserialize;
use serde_json::json;
use time::{Duration, OffsetDateTime};
use tracing::{info, warn};
use uuid::Uuid;

use crate::session_mw::{SessionContext, hash_token};
use crate::state::AppState;

const SESSION_TTL_DAYS: i64 = 30;
const RESET_TTL_MINUTES: i64 = 30;

#[derive(Deserialize)]
pub struct LoginBody {
    pub email: String,
    pub password: String,
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<LoginBody>,
) -> axum::response::Response {
    let row: Option<(Uuid, String, String)> = match sqlx::query_as(
        "SELECT id, password_hash, role FROM users WHERE LOWER(email) = LOWER($1)",
    )
    .bind(&body.email)
    .fetch_optional(&state.pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            warn!(error = %e, "login lookup failed");
            return internal();
        }
    };

    let ok = row.as_ref().is_some_and(|(_, phc, _)| {
        sentori_argon2_password::PasswordHash::verify(&body.password, phc).unwrap_or(false)
    });
    let Some((user_id, _, role)) = row else {
        // Burn comparable time so a timing probe cannot separate
        // unknown-email from wrong-password.
        let _ = sentori_argon2_password::PasswordHash::hash("timing-equalizer");
        return unauthorized();
    };
    if !ok {
        return unauthorized();
    }

    let token = mint_session_token();
    let hash = hash_token(&token);
    let expires = OffsetDateTime::now_utc() + Duration::days(SESSION_TTL_DAYS);
    let ip = crate::client_ip::client_ip(&headers);
    let ua = headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(std::borrow::ToOwned::to_owned);
    if let Err(e) = sqlx::query(
        "INSERT INTO auth_sessions (id_hash, user_id, expires_at, ip, user_agent) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(hash.as_slice())
    .bind(user_id)
    .bind(expires)
    .bind(ip.as_deref())
    .bind(ua.as_deref())
    .execute(&state.pool)
    .await
    {
        warn!(error = %e, "session insert failed");
        return internal();
    }
    let _ = sqlx::query("UPDATE users SET last_login_at = now() WHERE id = $1")
        .bind(user_id)
        .execute(&state.pool)
        .await;

    info!(%user_id, "auth.login");
    let cookie = format!(
        "sentori_session={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
        SESSION_TTL_DAYS * 86_400
    );
    (
        StatusCode::OK,
        [(axum::http::header::SET_COOKIE, cookie)],
        Json(json!({ "userId": user_id, "role": role, "sessionToken": token })),
    )
        .into_response()
}

pub async fn logout(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
) -> axum::response::Response {
    let _ = sqlx::query("DELETE FROM auth_sessions WHERE id_hash = $1")
        .bind(ctx.session_id_hash.as_slice())
        .execute(&state.pool)
        .await;
    let clear = "sentori_session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0";
    (
        StatusCode::OK,
        [(axum::http::header::SET_COOKIE, clear.to_string())],
        Json(json!({ "ok": true })),
    )
        .into_response()
}

pub async fn me(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
) -> (StatusCode, Json<serde_json::Value>) {
    let row: Option<(String, Option<String>)> =
        sqlx::query_as("SELECT email, display_name FROM users WHERE id = $1")
            .bind(ctx.user_id)
            .fetch_optional(&state.pool)
            .await
            .unwrap_or(None);
    match row {
        Some((email, display_name)) => (
            StatusCode::OK,
            Json(json!({
                "userId": ctx.user_id,
                "email": email,
                "displayName": display_name,
                "role": ctx.role.as_db_str(),
            })),
        ),
        None => (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "user_not_found" })),
        ),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePasswordBody {
    pub current_password: String,
    pub new_password: String,
}

pub async fn change_password(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Json(body): Json<ChangePasswordBody>,
) -> (StatusCode, Json<serde_json::Value>) {
    if body.new_password.len() < 8 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "password_too_short", "min": 8 })),
        );
    }
    let row: Option<(String,)> = sqlx::query_as("SELECT password_hash FROM users WHERE id = $1")
        .bind(ctx.user_id)
        .fetch_optional(&state.pool)
        .await
        .unwrap_or(None);
    let Some((phc,)) = row else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "user_not_found" })),
        );
    };
    if !sentori_argon2_password::PasswordHash::verify(&body.current_password, &phc).unwrap_or(false)
    {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "wrong_password" })),
        );
    }
    let Ok(new_phc) = sentori_argon2_password::PasswordHash::hash(&body.new_password) else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "hash_failed" })),
        );
    };
    match sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
        .bind(&new_phc)
        .bind(ctx.user_id)
        .execute(&state.pool)
        .await
    {
        Ok(_) => (StatusCode::OK, Json(json!({ "ok": true }))),
        Err(e) => {
            warn!(error = %e, "password update failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}

#[derive(Deserialize)]
pub struct ForgotBody {
    pub email: String,
}

/// Always 200 — whether the email exists is not disclosed. The
/// reset token goes out by email ONLY (or the operator log when
/// SMTP is unconfigured); returning it here would hand account
/// takeover to anyone who can POST.
pub async fn forgot_password(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ForgotBody>,
) -> (StatusCode, Json<serde_json::Value>) {
    let row: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM users WHERE LOWER(email) = LOWER($1)")
            .bind(&body.email)
            .fetch_optional(&state.pool)
            .await
            .unwrap_or(None);
    if let Some((user_id,)) = row {
        let token = mint_session_token();
        let hash = hash_token(&token);
        let expires = OffsetDateTime::now_utc() + Duration::minutes(RESET_TTL_MINUTES);
        let ok = sqlx::query(
            "INSERT INTO password_resets (id, user_id, token_hash, expires_at) \
             VALUES ($1, $2, $3, $4)",
        )
        .bind(Uuid::now_v7())
        .bind(user_id)
        .bind(hash.as_slice())
        .bind(expires)
        .execute(&state.pool)
        .await;
        if ok.is_ok() {
            state.mailer.send_reset(&body.email, &token);
        }
    }
    (StatusCode::OK, Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetBody {
    pub token: String,
    pub new_password: String,
}

pub async fn reset_password(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ResetBody>,
) -> (StatusCode, Json<serde_json::Value>) {
    if body.new_password.len() < 8 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "password_too_short", "min": 8 })),
        );
    }
    let hash = hash_token(&body.token);
    let row: Option<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT id, user_id FROM password_resets \
         WHERE token_hash = $1 AND used_at IS NULL AND expires_at > now()",
    )
    .bind(hash.as_slice())
    .fetch_optional(&state.pool)
    .await
    .unwrap_or(None);
    let Some((reset_id, user_id)) = row else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "invalid_or_expired_token" })),
        );
    };
    let Ok(new_phc) = sentori_argon2_password::PasswordHash::hash(&body.new_password) else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "hash_failed" })),
        );
    };
    let ok = sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
        .bind(&new_phc)
        .bind(user_id)
        .execute(&state.pool)
        .await;
    if ok.is_err() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "internal" })),
        );
    }
    let _ = sqlx::query("UPDATE password_resets SET used_at = now() WHERE id = $1")
        .bind(reset_id)
        .execute(&state.pool)
        .await;
    // Every live session for this user dies with the old password.
    let _ = sqlx::query("DELETE FROM auth_sessions WHERE user_id = $1")
        .bind(user_id)
        .execute(&state.pool)
        .await;
    (StatusCode::OK, Json(json!({ "ok": true })))
}

/// 32 random bytes, base32 — same entropy family as ingest tokens.
fn mint_session_token() -> String {
    use rand::Rng;
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    data_encoding::BASE32_NOPAD
        .encode(&bytes)
        .to_ascii_lowercase()
}

fn unauthorized() -> axum::response::Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({ "error": "invalid_credentials" })),
    )
        .into_response()
}

fn internal() -> axum::response::Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": "internal" })),
    )
        .into_response()
}
