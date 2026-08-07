//! Axum integration: middleware, extractor, cookie builder.
//!
//! Imports:
//!
//! ```ignore
//! use sentori_auth_session::{AuthService, AuthOptions, axum::{require_user, CurrentUser, build_session_cookie}};
//! use axum::{Router, routing::get, middleware};
//!
//! # async fn setup(auth: AuthService) -> Router {
//! Router::new()
//!     .route("/me", get(me_handler))
//!     .route_layer(middleware::from_fn_with_state(auth.clone(), require_user))
//!     .with_state(auth)
//! # }
//!
//! async fn me_handler(axum::Extension(user): axum::Extension<CurrentUser>) -> String {
//!     format!("Hello {}", user.email)
//! }
//! ```

use ::axum::extract::{Request, State};
use ::axum::http::StatusCode;
use ::axum::middleware::Next;
use ::axum::response::Response;
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use sentori_workspace_identity::UserId;
use time::Duration;

use crate::service::AuthService;
use crate::store::Session;

/// Inserted into request extensions by [`require_user`]. Handlers
/// pull it out via `axum::Extension<CurrentUser>`.
///
/// Carries enough to identify + scope authorization decisions
/// without re-fetching from DB. The full [`Session`] row is also
/// attached so handlers that need IP / last-seen / etc. can
/// reach for it.
#[derive(Clone, Debug)]
pub struct CurrentUser {
    /// Authenticated user id.
    pub id: UserId,
    /// Cached email (avoids a re-fetch in common UI flows).
    pub email: String,
    /// Whether the email has been verified.
    pub email_verified: bool,
    /// The active session row.
    pub session: Session,
}

impl CurrentUser {
    /// The SHA-256 hash of the session id as a raw 32-byte
    /// array. Used by handlers that need to call
    /// [`AuthService::sign_out_everywhere`] /
    /// [`AuthService::change_password`] keeping the current
    /// session alive.
    ///
    /// # Errors
    ///
    /// Returns an empty array on hex decode failure (should
    /// never happen — the hash was produced by us upstream).
    #[must_use]
    pub fn session_id_hash(&self) -> [u8; 32] {
        let bytes = hex_decode_32(&self.session.id_hash_hex);
        bytes.unwrap_or([0u8; 32])
    }
}

/// Middleware: extract the session cookie, look up the session,
/// 401 on failure or attach a [`CurrentUser`] extension and
/// forward.
///
/// Wire it as `from_fn_with_state(auth.clone(), require_user)`
/// — the state is the `AuthService`.
///
/// # Errors
///
/// Never errors directly; failures collapse to a 401 response.
pub async fn require_user(
    State(auth): State<AuthService>,
    jar: CookieJar,
    mut req: Request,
    next: Next,
) -> Response {
    let cookie_name = auth.opts().cookie_name;
    let cookie_value = match jar.get(cookie_name) {
        Some(c) => c.value().to_string(),
        None => return unauthorized(),
    };

    // Any non-Ok(Some) outcome collapses to 401. We don't leak
    // DB error detail to the client; ops sees it via the
    // tracing layer that wraps the handler upstream.
    let Ok(Some((user, session))) = auth.lookup_session(&cookie_value).await else {
        return unauthorized();
    };

    let current = CurrentUser {
        id: user.id,
        email: user.email,
        email_verified: user.email_verified,
        session,
    };
    req.extensions_mut().insert(current);
    next.run(req).await
}

fn unauthorized() -> Response {
    let mut resp = Response::new(::axum::body::Body::empty());
    *resp.status_mut() = StatusCode::UNAUTHORIZED;
    resp
}

/// Build a `Set-Cookie` value for a freshly minted session.
///
/// Wraps the wire-encoded `session_id` in an HMAC-signed S9
/// cookie (tamper-evident; the plaintext id is still visible
/// to the client, which is fine — it's an opaque token).
///
/// The cookie is `HttpOnly` (JS can't read it) + `SameSite=Lax`
/// (defends against CSRF on top-level navigations from third-
/// party sites) + `Secure` per [`crate::AuthOptions::cookie_secure`].
///
/// Build the cookie's max-age from the session's `expires_at`
/// to keep client expiry in sync with server expiry.
#[must_use]
pub fn build_session_cookie<'a>(
    auth: &AuthService,
    session_id_wire: &str,
    expires_at: time::OffsetDateTime,
) -> Cookie<'a> {
    let signed =
        sentori_cookie_session::SignedCookie::seal(auth.cookie_key(), session_id_wire.as_bytes());
    let max_age = expires_at - time::OffsetDateTime::now_utc();
    let max_age = if max_age.is_positive() {
        max_age
    } else {
        Duration::seconds(0)
    };
    Cookie::build((auth.opts().cookie_name.to_string(), signed))
        .path("/")
        .http_only(true)
        .secure(auth.opts().cookie_secure)
        .same_site(SameSite::Lax)
        .max_age(max_age)
        .build()
}

/// Build a `Set-Cookie` value that clears the session cookie.
///
/// Used by logout handlers — sets the same cookie name with
/// empty value and `max-age=0`.
#[must_use]
pub fn clear_session_cookie<'a>(auth: &AuthService) -> Cookie<'a> {
    Cookie::build((auth.opts().cookie_name.to_string(), String::new()))
        .path("/")
        .http_only(true)
        .secure(auth.opts().cookie_secure)
        .same_site(SameSite::Lax)
        .max_age(Duration::seconds(0))
        .build()
}

fn hex_decode_32(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, chunk) in s.as_bytes().chunks_exact(2).enumerate() {
        let hi = hex_nibble(chunk[0])?;
        let lo = hex_nibble(chunk[1])?;
        out[i] = (hi << 4) | lo;
    }
    Some(out)
}

const fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}
