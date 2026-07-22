//! Dashboard user auth — register / login / verify-email /
//! forgot+reset password / change password / logout.
//!
//! Phase E step 6 ships the API endpoints with JSON in/out.
//! Cookie-session middleware (axum_middleware::from_fn) lands
//! separately; for now login returns the session id plaintext
//! for the dashboard to store client-side (typical for v0.2 dev).

use std::sync::Arc;

use axum::{Json, extract::State, http::StatusCode};
use sentori_auth_session::{AuthOptions, AuthService, RequestMeta};
use sentori_cookie_session::SecretKey;
use sentori_workspace_identity::UserId;
use serde::Deserialize;
use serde_json::{Value, json};
use tracing::{info, warn};

use crate::state::AppState;

pub(crate) fn auth(state: &Arc<AppState>) -> AuthService {
    let raw = std::env::var("SENTORI_SESSION_SECRET").ok();
    let key = match raw {
        Some(s) if s.len() >= 32 => {
            let mut a = [0u8; 32];
            a.copy_from_slice(&s.as_bytes()[..32]);
            SecretKey::from_bytes(a)
        }
        _ => {
            warn!(
                "SENTORI_SESSION_SECRET missing or < 32 bytes; using ephemeral key (sessions reset on restart)"
            );
            // Only fails if the OS CSPRNG is unavailable, which no
            // request could be served through anyway. Threading a
            // Result out of here would change every caller's
            // signature, which is out of scope for a lint pass.
            #[allow(clippy::expect_used)]
            SecretKey::generate().expect("session key generate")
        }
    };
    AuthService::new(state.identity.clone(), key, AuthOptions::default())
}

fn meta() -> RequestMeta {
    RequestMeta {
        ip: None,
        user_agent: None,
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterBody {
    pub email: String,
    pub password: String,
}

pub async fn register(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RegisterBody>,
) -> (StatusCode, Json<Value>) {
    if body.email.is_empty() || body.password.len() < 12 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "email + password (≥12 chars) required" })),
        );
    }
    match auth(&state).register(&body.email, &body.password).await {
        Ok((user, minted, workspace_id)) => {
            info!(user_id = %user.id, %workspace_id, "auth.register");
            // Seed the new tenant's billing row (Free plan) so
            // quota checks + the usage page have a row to read.
            // Best-effort: a missing billing row degrades to
            // "no plan yet", not a failed signup.
            if let Err(e) = state.billing_for(workspace_id).ensure_default().await {
                warn!(error = %e, %workspace_id, "billing seed on signup failed");
            }
            // The verify token goes out by email ONLY — returning
            // it here would let any caller self-verify. Scoped to
            // the caller's freshly-minted workspace.
            state.mailer.send_verify(
                workspace_id,
                &body.email,
                &minted.plaintext_token.to_wire_string(),
            );
            (
                StatusCode::CREATED,
                Json(json!({
                    "user_id": user.id.to_string(),
                    "status": "verification email sent",
                })),
            )
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginBody {
    pub email: String,
    pub password: String,
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(body): Json<LoginBody>,
) -> axum::response::Response {
    use axum::http::header::{HeaderValue, SET_COOKIE};
    use axum::response::IntoResponse;

    let auth_svc = auth(&state);
    match auth_svc.login(&body.email, &body.password, &meta()).await {
        Ok((user, minted)) => {
            info!(user_id = %user.id, "auth.login");
            let raw = minted.session_id.to_wire_string();
            // Seal the raw wire token into the signed cookie form
            // that lookup_session expects. session_token in the body
            // is the signed value too so cli / Bearer clients can use
            // the exact same string they would put in the cookie.
            let signed =
                sentori_cookie_session::SignedCookie::seal(auth_svc.cookie_key(), raw.as_bytes());
            let body_json = json!({
                "user_id": user.id.to_string(),
                "email": user.email,
                "session_token": signed,
                "expires_at": crate::wire_time::rfc3339(minted.session.expires_at),
            });
            let mut resp = (StatusCode::OK, Json(body_json)).into_response();
            let cookie = session_cookie_header(&signed);
            if let Ok(hv) = HeaderValue::from_str(&cookie) {
                resp.headers_mut().insert(SET_COOKIE, hv);
            }
            resp
        }
        Err(e) => (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// The one place the session cookie's name and flags are written.
///
/// OAuth login (`handlers::oauth`) mints a session outside this
/// module and must set a byte-identical cookie, or the two login
/// paths drift apart the next time a flag changes here.
pub(crate) fn session_cookie_header(signed: &str) -> String {
    format!(
        "sentori_session={signed}; Path=/; HttpOnly; SameSite=Lax{}",
        if secure_cookies() { "; Secure" } else { "" },
    )
}

pub(crate) fn secure_cookies() -> bool {
    // Default ON; flip OFF for local-dev plain HTTP.
    !matches!(
        std::env::var("SENTORI_COOKIE_SECURE").ok().as_deref(),
        Some("0" | "false")
    )
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyBody {
    pub token: String,
}

pub async fn verify(
    State(state): State<Arc<AppState>>,
    Json(body): Json<VerifyBody>,
) -> (StatusCode, Json<Value>) {
    match auth(&state).verify_email(&body.token).await {
        Ok(uid) => (StatusCode::OK, Json(json!({ "user_id": uid.to_string() }))),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgotBody {
    pub email: String,
}

pub async fn forgot(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ForgotBody>,
) -> (StatusCode, Json<Value>) {
    match auth(&state).forgot_password(&body.email).await {
        // Same response for hit and miss (anti-enumeration), and
        // the token travels by email ONLY — returning it here
        // hands account takeover to any caller.
        Ok(minted) => {
            if let Some(minted) = minted {
                state.mailer.send_reset(
                    state.workspace_id,
                    &body.email,
                    &minted.plaintext_token.to_wire_string(),
                );
            }
            (
                StatusCode::OK,
                Json(json!({ "status": "if registered, an email is sent" })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetBody {
    pub token: String,
    pub new_password: String,
}

pub async fn reset(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ResetBody>,
) -> (StatusCode, Json<Value>) {
    match auth(&state)
        .reset_password(&body.token, &body.new_password)
        .await
    {
        Ok(uid) => (StatusCode::OK, Json(json!({ "user_id": uid.to_string() }))),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePasswordBody {
    pub user_id: uuid::Uuid,
    pub current_password: String,
    pub new_password: String,
    /// 32-byte session id hash (hex) of the calling session — kept
    /// alive after the rotate.
    pub keep_session_id_hex: String,
}

pub async fn logout(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> axum::response::Response {
    use axum::http::header::{HeaderValue, SET_COOKIE};
    use axum::response::IntoResponse;

    if let Some(token) = extract_session_token(&headers) {
        let svc = auth(&state);
        if let Ok(Some((_user, session))) = svc.lookup_session(&token).await
            && let Ok(hash) = hex::decode(&session.id_hash_hex)
            && hash.len() == 32
        {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&hash);
            let _ = svc.logout(&arr).await;
        }
    }
    let mut resp = StatusCode::NO_CONTENT.into_response();
    let cookie = format!(
        "sentori_session=; Path=/; HttpOnly; Max-Age=0; SameSite=Lax{}",
        if secure_cookies() { "; Secure" } else { "" },
    );
    if let Ok(hv) = HeaderValue::from_str(&cookie) {
        resp.headers_mut().insert(SET_COOKIE, hv);
    }
    resp
}

pub async fn me(
    axum::extract::Extension(ctx): axum::extract::Extension<crate::session_mw::SessionContext>,
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    match state.identity.users().find_by_id(ctx.user_id).await {
        Ok(Some(u)) => {
            // Active workspace name for the dashboard header. Best-
            // effort: a missing name just omits it, it does not fail
            // the whoami.
            let ws_name: Option<String> =
                sqlx::query_scalar("SELECT name FROM workspaces WHERE id = $1")
                    .bind(ctx.workspace_id.into_uuid())
                    .fetch_optional(&state.pool)
                    .await
                    .ok()
                    .flatten();
            Json(json!({
                "user_id": u.id.to_string(),
                "email": u.email,
                "email_verified": u.email_verified,
                "created_at": crate::wire_time::rfc3339(u.created_at),
                // Active-workspace context for the header + switcher.
                "workspace_id": ctx.workspace_id.into_uuid().to_string(),
                "workspace_name": ws_name,
                "role": ctx.role.as_db_str(),
                // Whether to show the cross-workspace SaaS operator
                // surface (`/saas`). The route is server-gated too;
                // this just hides the nav entry for non-operators.
                "is_saasadmin": crate::saasadmin_mw::is_saasadmin(ctx.user_id.into_uuid(), ctx.role),
            }))
        }
        _ => Json(json!({ "error": "user_not_found" })),
    }
}

fn extract_session_token(headers: &axum::http::HeaderMap) -> Option<String> {
    use axum::http::header;
    if let Some(auth) = headers.get(header::AUTHORIZATION)
        && let Ok(s) = auth.to_str()
        && let Some(rest) = s.strip_prefix("Bearer ")
    {
        return Some(rest.trim().to_string());
    }
    if let Some(cookie_hdr) = headers.get(header::COOKIE)
        && let Ok(s) = cookie_hdr.to_str()
    {
        for part in s.split(';') {
            let p = part.trim();
            if let Some(rest) = p.strip_prefix("sentori_session=") {
                return Some(rest.trim().to_string());
            }
        }
    }
    None
}

pub async fn change_password(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ChangePasswordBody>,
) -> (StatusCode, Json<Value>) {
    let keep = match hex::decode(&body.keep_session_id_hex) {
        Ok(b) if b.len() == 32 => {
            let mut a = [0u8; 32];
            a.copy_from_slice(&b);
            a
        }
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "keep_session_id_hex must be 32 bytes hex" })),
            );
        }
    };
    match auth(&state)
        .change_password(
            UserId::from_uuid(body.user_id),
            &body.current_password,
            &body.new_password,
            &keep,
        )
        .await
    {
        Ok(()) => (StatusCode::NO_CONTENT, Json(json!({}))),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}
