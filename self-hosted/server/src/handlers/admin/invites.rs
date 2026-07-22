//! Workspace invite admin endpoints:
//!
//! - `POST   /admin/api/invites` — mint invite token
//! - `GET    /admin/api/invites` — list all (pending + accepted +
//!   expired)
//! - `DELETE /admin/api/invites/:id` — revoke pending invite
//! - `POST   /auth/invites/:token/accept` — accepted by invitee
//!   (NB: this one is auth-scoped not admin; lives in auth/)

use std::sync::Arc;

use axum::{
    Json,
    extract::{Extension, Path, State},
    http::StatusCode,
};
use sentori_workspace_identity::InviteRole;
use serde::Deserialize;
use serde_json::{Value, json};
use tracing::{info, warn};
use uuid::Uuid;

use crate::session_mw::SessionContext;
use crate::state::AppState;

/// The inviter is not in this body.
///
/// It used to be, which meant the form asked an admin to type their own
/// uuid — and meant the field was whatever the client said it was, so
/// the invite audit trail recorded a claim rather than a fact. The
/// session already knows who is calling.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBody {
    pub email: String,
    pub role: String,
    /// Days until expiry (server-clamped to MAX_EXPIRES_IN_DAYS).
    #[serde(default = "default_expires")]
    pub expires_in_days: i64,
}

const fn default_expires() -> i64 {
    7
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Json(body): Json<CreateBody>,
) -> (StatusCode, Json<Value>) {
    // RBAC: inviting members is owner/admin only.
    if !ctx.role.can_manage_users() {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "insufficient_role" })),
        );
    }
    let role = match body.role.as_str() {
        "admin" => InviteRole::Admin,
        "user" => InviteRole::User,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "invalid_role" })),
            );
        }
    };
    match state
        .identity_for(ctx.workspace_id)
        .invites()
        .create(&body.email, role, ctx.user_id, body.expires_in_days)
        .await
    {
        Ok(minted) => {
            info!(
                invite_id = %minted.invite.id,
                email = %body.email,
                role = %body.role,
                "admin.invites minted",
            );
            crate::notify::audit(
                &state.pool,
                ctx.workspace_id.into_uuid(),
                None,
                Some(ctx.user_id.into_uuid()),
                "invite.mint",
                Some("invite"),
                Some(&minted.invite.id.to_string()),
                json!({ "email": body.email, "role": body.role }),
            )
            .await;
            (
                StatusCode::CREATED,
                Json(json!({
                    "invite_id": minted.invite.id.to_string(),
                    "token": minted.plaintext_token.to_wire_string(),
                    "expires_at": crate::wire_time::rfc3339(minted.invite.expires_at),
                })),
            )
        }
        Err(e) => {
            warn!(error = %e, "admin.invites create_failed");
            (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": e.to_string() })),
            )
        }
    }
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
) -> Json<Value> {
    match state
        .identity_for(ctx.workspace_id)
        .invites()
        .list_all()
        .await
    {
        Ok(rows) => {
            let out: Vec<Value> = rows
                .iter()
                .map(|i| {
                    json!({
                        "id": i.id.to_string(),
                        "email": i.email,
                        "role": match i.role {
                            InviteRole::Admin => "admin",
                            InviteRole::User => "user",
                        },
                        "expires_at": crate::wire_time::rfc3339(i.expires_at),
                        "accepted_at": crate::wire_time::rfc3339_opt(i.accepted_at),
                        "created_at": crate::wire_time::rfc3339(i.created_at),
                    })
                })
                .collect();
            Json(json!({ "invites": out }))
        }
        Err(e) => {
            warn!(error = %e, "admin.invites list_failed");
            Json(json!({ "invites": [], "error": "internal" }))
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptBody {
    pub token: String,
}

/// POST /admin/api/invites/accept — the logged-in caller joins the
/// workspace an invite token belongs to.
///
/// This is how the Team (1:N) model grows: an invited person signs
/// up (getting their own personal workspace), logs in, then accepts
/// — gaining a `workspace_members` row in the inviter's workspace,
/// reachable via the switcher. The token (from the emailed link) is
/// the authorization; it resolves the workspace, so the acceptor
/// never names it.
pub async fn accept(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Json(body): Json<AcceptBody>,
) -> (StatusCode, Json<Value>) {
    let ws = match sentori_workspace_identity::resolve_invite_workspace(&state.pool, &body.token)
        .await
    {
        Ok(Some(ws)) => ws,
        // Unknown / expired / already-accepted all collapse to 404.
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "invite invalid or expired" })),
            );
        }
        Err(e) => {
            warn!(error = %e, "admin.invites accept resolve_failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            );
        }
    };
    match state
        .identity_for(ws)
        .invites()
        .accept(&body.token, ctx.user_id)
        .await
    {
        Ok(member) => {
            info!(user_id = %ctx.user_id, workspace_id = %ws, "admin.invites accepted");
            (
                StatusCode::OK,
                Json(json!({
                    "workspace_id": ws.into_uuid().to_string(),
                    "role": member.role.as_db_str(),
                })),
            )
        }
        Err(e) => {
            warn!(error = %e, "admin.invites accept_failed");
            (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": e.to_string() })),
            )
        }
    }
}

pub async fn revoke(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(invite_id): Path<Uuid>,
) -> StatusCode {
    // RBAC: revoking invites is owner/admin only.
    if !ctx.role.can_manage_users() {
        return StatusCode::FORBIDDEN;
    }
    match state
        .identity_for(ctx.workspace_id)
        .invites()
        .revoke(invite_id)
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
