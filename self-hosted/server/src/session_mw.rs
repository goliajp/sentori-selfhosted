//! Cookie / Bearer session middleware for dashboard + admin
//! routes.
//!
//! Resolves the session token from either:
//! 1. `Authorization: Bearer <session_token_wire>` header
//! 2. `Cookie: sentori_session=<session_token_wire>`
//!
//! On success injects `Extension<SessionContext { user_id }>`
//! into the request. On failure returns 401 with `WWW-Authenticate`.

use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use sentori_auth_session::{AuthOptions, AuthService};
use sentori_cookie_session::SecretKey;
use sentori_workspace_identity::{Members, Role, UserId};
use serde_json::json;
use tracing::warn;

use crate::state::AppState;

/// Who the caller is, which workspace their request is acting in,
/// and the role they hold there.
///
/// `workspace_id` is the session's *active* workspace (a user can
/// belong to many; the switcher UPDATEs which one is active). It is
/// resolved + membership-validated here rather than left to each
/// handler: dashboard queries used to select across the whole
/// table, so any authenticated user saw every tenant's projects,
/// events, spans, metrics and replays. Carrying the scope on the
/// request makes the filter something a handler has to actively
/// drop rather than something it has to remember to add.
///
/// `role` is the caller's RBAC role in the active workspace — the
/// membership check that authorizes the request already fetched it,
/// so handlers can gate mutations (e.g. `role.can_manage_workspace()`)
/// without a second query.
#[derive(Clone, Copy, Debug)]
pub struct SessionContext {
    pub user_id: UserId,
    pub workspace_id: sentori_workspace_identity::WorkspaceId,
    pub role: Role,
    /// SHA-256 of the current session id. The workspace switcher
    /// needs it to UPDATE this exact session's active workspace;
    /// carrying it here avoids re-parsing the cookie in the
    /// handler. Zero-filled only if the hex failed to decode,
    /// which cannot happen for a session that just looked up.
    pub session_id_hash: [u8; 32],
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

    let auth = build_auth(&state);
    match auth.lookup_session(&token).await {
        Ok(Some((user, session))) => {
            // The session carries its *active* workspace. Validate
            // the user still belongs to it: a revoked membership (or
            // any stale active workspace) must be rejected, not
            // trusted — otherwise removing someone from a workspace
            // would leave their live sessions with access.
            let workspace_id = session.workspace_id;
            match Members::new(&state.pool, workspace_id).find(user.id).await {
                Ok(Some(member)) => {
                    req.extensions_mut().insert(SessionContext {
                        user_id: user.id,
                        workspace_id,
                        role: member.role,
                        session_id_hash: decode_hash(&session.id_hash_hex),
                    });
                    next.run(req).await
                }
                Ok(None) => reject("workspace access revoked"),
                Err(e) => {
                    warn!(error = %e, "membership check failed");
                    reject("internal")
                }
            }
        }
        Ok(None) => reject("session expired or invalid"),
        Err(e) => {
            warn!(error = %e, "session lookup failed");
            reject("internal")
        }
    }
}

/// Decode a 64-char hex session-id hash back into 32 bytes. The
/// value comes straight from `Session::id_hash_hex`, which we
/// produced from a real 32-byte hash, so a malformed string is not
/// reachable in practice; we return zeroes rather than panic so a
/// freak decode failure downgrades to "switcher can't target this
/// session" instead of a 500 on every request.
fn decode_hash(hex: &str) -> [u8; 32] {
    let mut out = [0u8; 32];
    if hex.len() == 64 {
        for (i, chunk) in hex.as_bytes().chunks_exact(2).enumerate() {
            let hi = (chunk[0] as char).to_digit(16);
            let lo = (chunk[1] as char).to_digit(16);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                // hi, lo are single hex nibbles (0..=15), so the
                // assembled byte always fits u8; unwrap_or is dead.
                out[i] = u8::try_from((hi << 4) | lo).unwrap_or(0);
            }
        }
    }
    out
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

fn build_auth(state: &Arc<AppState>) -> AuthService {
    let raw = std::env::var("SENTORI_SESSION_SECRET").ok();
    let key = match raw {
        Some(s) if s.len() >= 32 => {
            let mut a = [0u8; 32];
            a.copy_from_slice(&s.as_bytes()[..32]);
            SecretKey::from_bytes(a)
        }
        // Only fails if the OS CSPRNG is unavailable, which no request
        // could be served through anyway.
        #[allow(clippy::expect_used)]
        _ => SecretKey::generate().expect("ephemeral session key"),
    };
    AuthService::new(state.identity.clone(), key, AuthOptions::default())
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
