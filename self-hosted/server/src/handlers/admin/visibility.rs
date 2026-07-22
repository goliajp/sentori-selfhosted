//! Per-project access for the `user` role.
//!
//! `project_user_visibility` and its store have existed since the
//! identity crate was written, and nothing in this server had ever
//! reached them. The consequence was not a missing feature but a
//! missing boundary: `GET /v1/projects` returned every project in the
//! workspace to every member, so the role documented as "sees only
//! what it is granted" saw everything, and a project id is the key the
//! rest of this API addresses data by.
//!
//! Closing that in the list handler alone would have swapped one wrong
//! answer for another — a `user` who can never be granted anything is
//! not scoped, just useless. These are the endpoints that make the
//! role work.
//!
//! Owners and admins are refused a grant rather than silently
//! succeeding: they already see every project, so a row for them would
//! be a lie the next reader has to reason about. The crate returns
//! `VisibilityRefusedForElevatedRole` for exactly this.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Extension, Path, State},
    http::StatusCode,
};
use sentori_workspace_identity::{IdentityError, ProjectId, UserId};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::session_mw::SessionContext;
use crate::state::AppState;

type ApiResult = Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)>;

fn err(code: StatusCode, msg: &str) -> (StatusCode, Json<Value>) {
    (code, Json(json!({ "error": msg })))
}

/// Managing who can see what is the same right as managing members.
fn guard(ctx: &SessionContext) -> Result<(), (StatusCode, Json<Value>)> {
    if ctx.role.can_manage_users() {
        return Ok(());
    }
    Err(err(
        StatusCode::FORBIDDEN,
        "granting project access needs the owner or admin role",
    ))
}

fn translate(e: &IdentityError) -> (StatusCode, Json<Value>) {
    match e {
        IdentityError::VisibilityRefusedForElevatedRole => err(
            StatusCode::CONFLICT,
            "owners and admins already see every project; no grant is needed",
        ),
        IdentityError::ProjectNotFound(_) => err(StatusCode::NOT_FOUND, "project not found"),
        other => err(StatusCode::INTERNAL_SERVER_ERROR, &other.to_string()),
    }
}

/// `GET /admin/api/projects/{project_id}/visibility`
pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
) -> ApiResult {
    guard(&ctx)?;
    super::super::tenant::guard_project(&state, ctx.workspace_id, project_id)
        .await
        .map_err(|(s, m)| (s, Json(json!({ "error": m }))))?;

    let users = state
        .identity_for(ctx.workspace_id)
        .visibility()
        .list_for_project(ProjectId::from_uuid(project_id))
        .await
        .map_err(|e| translate(&e))?;

    Ok((
        StatusCode::OK,
        Json(json!({
            "user_ids": users.into_iter().map(|u| u.into_uuid().to_string()).collect::<Vec<_>>(),
        })),
    ))
}

/// `PUT /admin/api/projects/{project_id}/visibility/{user_id}`
pub async fn grant(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path((project_id, user_id)): Path<(Uuid, Uuid)>,
) -> ApiResult {
    guard(&ctx)?;
    super::super::tenant::guard_project(&state, ctx.workspace_id, project_id)
        .await
        .map_err(|(s, m)| (s, Json(json!({ "error": m }))))?;

    state
        .identity_for(ctx.workspace_id)
        .visibility()
        .grant(
            ProjectId::from_uuid(project_id),
            UserId::from_uuid(user_id),
            ctx.user_id,
        )
        .await
        .map_err(|e| translate(&e))?;

    Ok((StatusCode::NO_CONTENT, Json(json!({}))))
}

/// `DELETE /admin/api/projects/{project_id}/visibility/{user_id}`
pub async fn revoke(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path((project_id, user_id)): Path<(Uuid, Uuid)>,
) -> ApiResult {
    guard(&ctx)?;
    super::super::tenant::guard_project(&state, ctx.workspace_id, project_id)
        .await
        .map_err(|(s, m)| (s, Json(json!({ "error": m }))))?;

    state
        .identity_for(ctx.workspace_id)
        .visibility()
        .revoke(ProjectId::from_uuid(project_id), UserId::from_uuid(user_id))
        .await
        .map_err(|e| translate(&e))?;

    Ok((StatusCode::NO_CONTENT, Json(json!({}))))
}
