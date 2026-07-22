//! Saasadmin role middleware — further restricts /admin/api/saas/*
//! to a configured set of user_ids.
//!
//! In SaaS deployments, only a small number of operator accounts
//! should see the cross-workspace view. Regular workspace users
//! who happen to be logged in shouldn't be able to enumerate
//! other tenants.
//!
//! v0.2 step: env-var driven (`SENTORI_SAASADMIN_USER_IDS` — comma-
//! separated UUIDs). A future commit could promote this to a
//! `saasadmin_users` table.

use axum::{
    body::Body,
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use sentori_workspace_identity::Role;
use serde_json::json;
use uuid::Uuid;

use crate::session_mw::SessionContext;

pub async fn saasadmin_only(req: Request<Body>, next: Next) -> Response {
    let Some(ctx) = req.extensions().get::<SessionContext>().copied() else {
        return reject("session context missing — session middleware must run first");
    };
    if !is_saasadmin(ctx.user_id.into_uuid(), ctx.role) {
        return reject("saasadmin role required");
    }
    next.run(req).await
}

/// Whether a caller may use the cross-workspace SaaS operator
/// surface.
///
/// Two modes, distinguished by whether the allowlist env is set:
///
/// - **SaaS** (`SENTORI_SAASADMIN_USER_IDS` present): ONLY the
///   listed operator user-ids qualify. A tenant who owns their own
///   workspace is deliberately not an operator — otherwise every
///   customer could enumerate every other tenant. This is the
///   production path.
/// - **Self-hosted** (env unset): the single deployment's `owner`
///   is the de-facto operator; invited `admin` / `user` members are
///   not. Previously this path defaulted *open to any logged-in
///   user*, which leaked the cross-workspace view to invited
///   members — the role gate closes that.
#[must_use]
pub fn is_saasadmin(user_id: Uuid, role: Role) -> bool {
    match std::env::var("SENTORI_SAASADMIN_USER_IDS") {
        Ok(raw) => raw
            .split(',')
            .filter_map(|s| Uuid::parse_str(s.trim()).ok())
            .any(|u| u == user_id),
        Err(_) => matches!(role, Role::Owner),
    }
}

fn reject(reason: &str) -> Response {
    let body = json!({ "error": "forbidden", "reason": reason });
    (StatusCode::FORBIDDEN, axum::Json(body)).into_response()
}
