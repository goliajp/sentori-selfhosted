//! Cookie / Bearer session middleware for dashboard + admin routes.
//!
//! Resolves the session token from either:
//! 1. `Authorization: Bearer <token>` header
//! 2. `Cookie: sentori_session=<token>`
//!
//! The token is an opaque random string; the DB stores its SHA-256
//! (`auth_sessions.id_hash`), so a dump of the table cannot forge a
//! session. Looking the hash up IS the validation — no HMAC layer,
//! because there is nothing to verify offline in a single-tenant
//! server that owns its own session table.
//!
//! On success injects `Extension<SessionContext>`. On failure
//! returns 401 with `WWW-Authenticate`.

use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde_json::json;
use sha2::{Digest, Sha256};
use tracing::warn;
use uuid::Uuid;

use crate::state::AppState;

/// The two roles that exist (design.md §9). Superadmin sees and
/// manages everything; admin sees assigned projects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Superadmin,
    Admin,
}

impl Role {
    #[must_use]
    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "superadmin" => Some(Self::Superadmin),
            "admin" => Some(Self::Admin),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::Superadmin => "superadmin",
            Self::Admin => "admin",
        }
    }

    #[must_use]
    pub fn is_superadmin(self) -> bool {
        matches!(self, Self::Superadmin)
    }
}

/// Who the caller is and what they may do.
#[derive(Clone, Copy, Debug)]
pub struct SessionContext {
    pub user_id: Uuid,
    pub role: Role,
    /// SHA-256 of the current session token — lets a handler target
    /// exactly this session (logout, session list highlighting)
    /// without re-parsing the cookie.
    pub session_id_hash: [u8; 32],
}

/// SHA-256 of a wire session token — the shape stored in
/// `auth_sessions.id_hash`.
#[must_use]
pub fn hash_token(token: &str) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(token.as_bytes());
    h.finalize().into()
}

pub async fn session_middleware(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    let Some(token) = extract_token(&headers) else {
        return reject("session token missing");
    };
    let hash = hash_token(&token);

    let row: Result<Option<(Uuid, String)>, sqlx::Error> = sqlx::query_as(
        "SELECT u.id, u.role FROM auth_sessions s \
         JOIN users u ON u.id = s.user_id \
         WHERE s.id_hash = $1 AND s.expires_at > now()",
    )
    .bind(hash.as_slice())
    .fetch_optional(&state.pool)
    .await;

    match row {
        Ok(Some((user_id, role_str))) => {
            let Some(role) = Role::from_db_str(&role_str) else {
                warn!(%user_id, role = %role_str, "unknown role in users table");
                return reject("internal");
            };
            // Sliding freshness, best-effort: a failed touch must not
            // fail the request it rode in on.
            let _ = sqlx::query("UPDATE auth_sessions SET last_seen_at = now() WHERE id_hash = $1")
                .bind(hash.as_slice())
                .execute(&state.pool)
                .await;
            req.extensions_mut().insert(SessionContext {
                user_id,
                role,
                session_id_hash: hash,
            });
            next.run(req).await
        }
        Ok(None) => reject("session expired or invalid"),
        Err(e) => {
            warn!(error = %e, "session lookup failed");
            reject("internal")
        }
    }
}

fn extract_token(headers: &HeaderMap) -> Option<String> {
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

fn reject(reason: &str) -> Response {
    let body = json!({ "error": "unauthorized", "reason": reason });
    let mut resp = (StatusCode::UNAUTHORIZED, axum::Json(body)).into_response();
    resp.headers_mut().insert(
        header::WWW_AUTHENTICATE,
        header::HeaderValue::from_static("Bearer realm=\"sentori\""),
    );
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_round_trips() {
        for r in [Role::Superadmin, Role::Admin] {
            assert_eq!(Role::from_db_str(r.as_db_str()), Some(r));
        }
        assert_eq!(Role::from_db_str("owner"), None);
    }

    #[test]
    fn token_hash_is_stable_and_token_sensitive() {
        assert_eq!(hash_token("abc"), hash_token("abc"));
        assert_ne!(hash_token("abc"), hash_token("abd"));
    }

    #[test]
    fn extract_prefers_bearer_over_cookie() {
        let mut h = HeaderMap::new();
        h.insert(
            header::AUTHORIZATION,
            "Bearer tok-a".parse().unwrap_or_else(|_| unreachable!()),
        );
        h.insert(
            header::COOKIE,
            "sentori_session=tok-b"
                .parse()
                .unwrap_or_else(|_| unreachable!()),
        );
        assert_eq!(extract_token(&h).as_deref(), Some("tok-a"));
    }
}
