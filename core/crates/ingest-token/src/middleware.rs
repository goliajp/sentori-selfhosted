//! Axum middleware:`Authorization: Bearer st_pk_...` → inject
//! `(ProjectId, WorkspaceId)` as request extensions.

use axum::{
    Json,
    extract::{Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::Serialize;

use crate::error::TokenError;
use crate::model::TokenKind;
use crate::parse::looks_like_token;
use crate::store::TokenStore;

/// Per-request context injected by [`bearer_middleware`].
/// Handler extractors pull this from `Extension<IngestContext>`.
#[derive(Debug, Clone, Copy)]
pub struct IngestContext {
    /// The `tokens` row this request authenticated with. Carried so a
    /// caller can attribute or limit per credential rather than per
    /// project — two apps under one project hold separate tokens and
    /// should not share an allowance.
    pub token_id: uuid::Uuid,
    pub workspace_id: sentori_workspace_identity::WorkspaceId,
    pub project_id: sentori_workspace_identity::ProjectId,
    pub token_kind: TokenKind,
}

/// Axum middleware. Requires `Extension<TokenStore>` set on the
/// router. On success, attaches `IngestContext` to the request.
/// On failure, returns 401 JSON with a `hint` field.
pub async fn bearer_middleware(
    State(store): State<TokenStore>,
    headers: HeaderMap,
    mut req: Request,
    next: Next,
) -> Response {
    match resolve_token(&store, &headers).await {
        Ok(ctx) => {
            req.extensions_mut().insert(ctx);
            next.run(req).await
        }
        Err(e) => unauthorized(&e),
    }
}

async fn resolve_token(
    store: &TokenStore,
    headers: &HeaderMap,
) -> Result<IngestContext, TokenError> {
    let auth = headers
        .get(header::AUTHORIZATION)
        .ok_or(TokenError::MissingHeader)?
        .to_str()
        .map_err(|_| TokenError::MalformedHeader)?;

    let plaintext = auth
        .strip_prefix("Bearer ")
        .ok_or(TokenError::MalformedHeader)?
        .trim();

    if !looks_like_token(plaintext) {
        return Err(TokenError::WrongPrefix);
    }

    let token = store
        .lookup_by_plaintext(plaintext)
        .await?
        .ok_or(TokenError::NotFound)?;

    if !token.is_active() {
        return Err(TokenError::NotFound);
    }

    Ok(IngestContext {
        token_id: token.id,
        workspace_id: token.workspace_id,
        project_id: token.project_id,
        token_kind: token.kind,
    })
}

#[derive(Serialize)]
struct UnauthorizedBody<'a> {
    error: &'static str,
    hint: &'a str,
}

fn unauthorized(err: &TokenError) -> Response {
    let body = UnauthorizedBody {
        error: "unauthorized",
        hint: err.user_hint(),
    };
    (StatusCode::UNAUTHORIZED, Json(body)).into_response()
}
