//! Token admin endpoints:
//!
//! - `POST   /admin/api/projects/:project_id/tokens` — mint
//! - `GET    /admin/api/projects/:project_id/tokens` — list
//! - `DELETE /admin/api/tokens/:token_id` — revoke
//!
//! These are the new-customer onboarding entry point: SaaS user
//! signs up → creates project → mints a token here → pastes it
//! into their SDK `init({ token, ingestUrl })`. Self-hosted users
//! do the same via the dashboard after first-owner bootstrap.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Extension, Path, State},
    http::{HeaderMap, StatusCode},
};

use crate::session_mw::SessionContext;
use sentori_ingest_token::{TokenKind, TokenStore};
use sentori_workspace_identity::ProjectId;
use serde::Deserialize;
use serde_json::{Value, json};
use tracing::{info, warn};
use uuid::Uuid;

use crate::state::AppState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBody {
    /// Display label (e.g. "production iOS", "qa android").
    #[serde(default)]
    pub label: Option<String>,
    /// `public` (default — SDK ingest) or `admin` (server-side).
    #[serde(default)]
    pub kind: Option<String>,
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
    headers: HeaderMap,
    Json(body): Json<CreateBody>,
) -> (StatusCode, Json<Value>) {
    let kind = match body.kind.as_deref() {
        None | Some("public") => TokenKind::Public,
        Some("admin") => TokenKind::Admin,
        Some(other) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "invalid_kind", "got": other })),
            );
        }
    };
    // Tenant guard: the project in the path must belong to the
    // caller's workspace, or minting would attach a token to a
    // foreign project.
    if let Err((code, msg)) =
        crate::handlers::tenant::guard_project(&state, ctx.workspace_id, project_id).await
    {
        return (code, Json(json!({ "error": msg })));
    }
    let store = TokenStore::new(state.pool.clone());
    match store
        .create(
            ctx.workspace_id,
            ProjectId::from_uuid(project_id),
            kind,
            body.label.as_deref(),
        )
        .await
    {
        Ok((id, plaintext)) => {
            info!(
                workspace_id = %ctx.workspace_id,
                %project_id,
                token_id = %id,
                kind = ?kind,
                "admin.tokens minted",
            );
            let (ip, ua) = crate::notify::extract_request_meta(&headers);
            crate::notify::audit(
                &state.pool,
                ctx.workspace_id.into_uuid(),
                Some(project_id),
                Some(ctx.user_id.into_uuid()),
                "token.mint",
                Some("token"),
                Some(&id.to_string()),
                crate::notify::enrich_payload(
                    json!({ "kind": kind.as_db_str(), "label": body.label }),
                    ip.as_deref(),
                    ua.as_deref(),
                ),
            )
            .await;
            (
                StatusCode::CREATED,
                Json(json!({
                    "token_id": id.to_string(),
                    "token": plaintext,
                    "kind": kind.as_db_str(),
                    "label": body.label,
                })),
            )
        }
        Err(e) => {
            warn!(error = %e, "admin.tokens create_failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
) -> (StatusCode, Json<Value>) {
    // Tenant guard: only list tokens for a project the caller owns.
    if let Err((code, msg)) =
        crate::handlers::tenant::guard_project(&state, ctx.workspace_id, project_id).await
    {
        return (code, Json(json!({ "error": msg })));
    }
    let store = TokenStore::new(state.pool.clone());
    match store
        .list_for_project(ProjectId::from_uuid(project_id))
        .await
    {
        Ok(rows) => {
            let out: Vec<Value> = rows
                .iter()
                .map(|t| {
                    json!({
                        "id": t.id.to_string(),
                        "kind": t.kind.as_db_str(),
                        "label": t.label,
                        "last4": t.last4,
                        "created_at": crate::wire_time::rfc3339(t.created_at),
                        "revoked_at": crate::wire_time::rfc3339_opt(t.revoked_at),
                    })
                })
                .collect();
            (StatusCode::OK, Json(json!({ "tokens": out })))
        }
        Err(e) => {
            warn!(error = %e, "admin.tokens list_failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "tokens": [], "error": "internal" })),
            )
        }
    }
}

pub async fn revoke(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(token_id): Path<Uuid>,
    headers: HeaderMap,
) -> StatusCode {
    // Tenant guard: the token must belong to the caller's
    // workspace. `TokenStore::revoke` keys on id alone, so without
    // this a caller could revoke any workspace's token by id.
    let owned: Result<Option<(Uuid,)>, _> =
        sqlx::query_as("SELECT id FROM tokens WHERE id = $1 AND workspace_id = $2")
            .bind(token_id)
            .bind(ctx.workspace_id.into_uuid())
            .fetch_optional(&state.pool)
            .await;
    match owned {
        Ok(Some(_)) => {}
        // Absent or foreign: 404, indistinguishable.
        Ok(None) => return StatusCode::NOT_FOUND,
        Err(e) => {
            warn!(error = %e, "admin.tokens revoke guard failed");
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    }
    let store = TokenStore::new(state.pool.clone());
    match store.revoke(token_id).await {
        Ok(()) => {
            info!(%token_id, "admin.tokens revoked");
            let (ip, ua) = crate::notify::extract_request_meta(&headers);
            crate::notify::audit(
                &state.pool,
                ctx.workspace_id.into_uuid(),
                None,
                Some(ctx.user_id.into_uuid()),
                "token.revoke",
                Some("token"),
                Some(&token_id.to_string()),
                crate::notify::enrich_payload(json!({}), ip.as_deref(), ua.as_deref()),
            )
            .await;
            StatusCode::NO_CONTENT
        }
        Err(e) => {
            warn!(error = %e, %token_id, "admin.tokens revoke_failed");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}
