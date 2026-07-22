//! Dashboard workspace switcher (multi-workspace, 1:N).
//!
//! v0.2 lets a user belong to several workspaces and switch which
//! one their session is acting in. Two session-gated endpoints back
//! the switcher UI:
//!
//! - `GET  /admin/api/workspaces`        — list the caller's memberships
//! - `POST /admin/api/workspaces/switch` — repoint the session
//!
//! Switching UPDATEs `auth_sessions.workspace_id` for the current
//! session; `session_mw` re-validates the (user, workspace) pair on
//! every subsequent request, so the switch cannot outlive the
//! caller's membership.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Extension, State},
    http::StatusCode,
};
use sentori_auth_session::Sessions;
use sentori_workspace_identity::{Members, WorkspaceId};
use serde::Deserialize;
use serde_json::{Value, json};
use tracing::{info, warn};
use uuid::Uuid;

use crate::session_mw::SessionContext;
use crate::state::AppState;

/// GET /admin/api/workspaces — every workspace the caller belongs
/// to, each with the role they hold and whether it is the session's
/// active one.
pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
) -> (StatusCode, Json<Value>) {
    // `list_for_user` is cross-workspace; the scope arg is ignored.
    match Members::new(&state.pool, ctx.workspace_id)
        .list_for_user(ctx.user_id)
        .await
    {
        Ok(rows) => {
            let out: Vec<Value> = rows
                .iter()
                .map(|w| {
                    json!({
                        "workspace_id": w.workspace_id.into_uuid().to_string(),
                        "name": w.name,
                        "role": w.role.as_db_str(),
                        "active": w.workspace_id == ctx.workspace_id,
                    })
                })
                .collect();
            (StatusCode::OK, Json(json!({ "workspaces": out })))
        }
        Err(e) => {
            warn!(error = %e, "workspaces.list failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchBody {
    pub workspace_id: Uuid,
}

/// POST /admin/api/workspaces/switch — point the current session at
/// a different workspace the caller belongs to.
pub async fn switch(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Json(body): Json<SwitchBody>,
) -> (StatusCode, Json<Value>) {
    let target = WorkspaceId::from_uuid(body.workspace_id);

    // Authorize: caller must be a member of the target. A non-member
    // gets 404 (same answer as an unknown workspace) so the endpoint
    // is not an oracle for which workspace ids exist.
    match Members::new(&state.pool, target).find(ctx.user_id).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "workspace not found" })),
            );
        }
        Err(e) => {
            warn!(error = %e, "workspaces.switch membership check failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            );
        }
    }

    match Sessions::new(&state.pool)
        .set_active_workspace(&ctx.session_id_hash, target)
        .await
    {
        Ok(true) => {
            info!(user_id = %ctx.user_id, workspace_id = %target, "workspaces.switch");
            (
                StatusCode::OK,
                Json(json!({ "workspace_id": target.into_uuid().to_string() })),
            )
        }
        Ok(false) => (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "session expired" })),
        ),
        Err(e) => {
            warn!(error = %e, "workspaces.switch set_active failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}
