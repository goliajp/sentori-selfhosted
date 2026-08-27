//! Token management (owner + assigned admins).
//!
//! Multiple named tokens per project, two scopes (design.md §9):
//! `ingest` for SDKs, `api` for automation / AI agents. Rotation is
//! create-new → switch clients → revoke-old; the plaintext appears
//! exactly once, in the create response.

use std::sync::Arc;

use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};
use sentori_ingest_token::{Scope, TokenStore};
use serde::Deserialize;
use serde_json::{Value, json};
use tracing::warn;
use uuid::Uuid;

use crate::session_mw::SessionContext;
use crate::state::AppState;

/// 403 unless the caller may touch this project (superadmin, or an
/// admin with an assignment row).
pub async fn ensure_project_access(
    state: &Arc<AppState>,
    ctx: &SessionContext,
    project_id: Uuid,
) -> Result<(), (StatusCode, Json<Value>)> {
    if ctx.role.is_superadmin() {
        return Ok(());
    }
    let assigned: Option<(i32,)> =
        sqlx::query_as("SELECT 1 FROM project_assignments WHERE user_id = $1 AND project_id = $2")
            .bind(ctx.user_id)
            .bind(project_id)
            .fetch_optional(&state.pool)
            .await
            .unwrap_or(None);
    if assigned.is_some() {
        Ok(())
    } else {
        Err((
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "project_access_denied" })),
        ))
    }
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
) -> (StatusCode, Json<Value>) {
    if let Err(e) = ensure_project_access(&state, &ctx, project_id).await {
        return e;
    }
    match TokenStore::new(state.pool.clone())
        .list_for_project(project_id)
        .await
    {
        Ok(tokens) => {
            let rows: Vec<Value> = tokens
                .iter()
                .map(|t| {
                    json!({
                        "id": t.id,
                        "name": t.name,
                        "scope": t.scope.as_db_str(),
                        "last4": t.last4,
                        "createdAt": crate::wire_time::rfc3339(t.created_at),
                        "revokedAt": t.revoked_at.map(crate::wire_time::rfc3339),
                    })
                })
                .collect();
            (StatusCode::OK, Json(json!({ "tokens": rows })))
        }
        Err(e) => {
            warn!(error = %e, "token list failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}

#[derive(Deserialize)]
pub struct CreateBody {
    pub name: String,
    pub scope: String,
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreateBody>,
) -> (StatusCode, Json<Value>) {
    if let Err(e) = ensure_project_access(&state, &ctx, project_id).await {
        return e;
    }
    let Some(scope) = Scope::from_db_str(&body.scope) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "invalid_scope", "hint": "ingest | api" })),
        );
    };
    match TokenStore::new(state.pool.clone())
        .create(project_id, scope, &body.name)
        .await
    {
        Ok((id, plaintext)) => {
            crate::audit::record(
                &state.pool,
                Some(project_id),
                ctx.user_id,
                "token.create",
                "token",
                &id.to_string(),
                json!({ "name": body.name, "scope": body.scope }),
            )
            .await;
            (
                StatusCode::CREATED,
                Json(json!({ "id": id, "token": plaintext })),
            )
        }
        Err(e) => {
            warn!(error = %e, "token create failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}

pub async fn revoke(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(token_id): Path<Uuid>,
) -> (StatusCode, Json<Value>) {
    // Resolve the token's project for the access check.
    let row: Option<(Uuid,)> = sqlx::query_as("SELECT project_id FROM tokens WHERE id = $1")
        .bind(token_id)
        .fetch_optional(&state.pool)
        .await
        .unwrap_or(None);
    let Some((project_id,)) = row else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "token_not_found" })),
        );
    };
    if let Err(e) = ensure_project_access(&state, &ctx, project_id).await {
        return e;
    }
    match TokenStore::new(state.pool.clone()).revoke(token_id).await {
        Ok(()) => {
            crate::audit::record(
                &state.pool,
                Some(project_id),
                ctx.user_id,
                "token.revoke",
                "token",
                &token_id.to_string(),
                json!({}),
            )
            .await;
            (StatusCode::OK, Json(json!({ "ok": true })))
        }
        Err(e) => {
            warn!(error = %e, "token revoke failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}
