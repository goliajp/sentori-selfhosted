//! SDK ingest endpoint handlers.
//!
//! All endpoints in this module are gated by `bearer_middleware`
//! (Bearer st_pk_<26 base32> Authorization header). Each handler
//! receives `Extension<IngestContext>` with the resolved
//! `(workspace_id, project_id, token_kind)`.
//!
//! Phase C step 2: stubs accept the legacy SDK wire format
//! (deserialized as `serde_json::Value` for now), log the call,
//! and return 202 Accepted with minimal response shape. Phase C
//! step 3+ replaces each stub body with the actual service-crate
//! integration (event-pipeline / span-store / etc).

pub mod deploys;
pub mod events;
pub mod events_attachments;
pub mod events_batch;
pub mod events_validate;
pub mod push;

/// Reject a public token on an endpoint a shipped application has no
/// business calling.
///
/// A public token is compiled into the customer's app; anyone holding
/// the app holds the token. Endpoints that write build artifacts, send
/// notifications to the customer's users, or stream their event feed
/// need the server-side kind instead.
///
/// 403 rather than 401: the credential is valid, it is simply not
/// allowed here, and telling the caller to re-authenticate would send
/// them chasing the wrong problem.
pub(crate) fn require_admin_token(
    ctx: &sentori_ingest_token::IngestContext,
) -> Result<(), (axum::http::StatusCode, axum::Json<serde_json::Value>)> {
    if ctx.scope == sentori_ingest_token::Scope::Api {
        return Ok(());
    }
    Err((
        axum::http::StatusCode::FORBIDDEN,
        axum::Json(serde_json::json!({
            // The code stays as it is: `docs/protocol.md` documents
            // it and the CLI's tests match on it. The hint is the
            // part that misled — it named a "kind `admin`" and a
            // token that is "`public`", and neither word appears
            // anywhere a reader can act on. The scopes are `ingest`
            // and `api`, which is what the UI offers.
            "error": "admin_token_required",
            "hint": "this endpoint needs a token whose scope is `api`. The token \
                     used has scope `ingest` — the one that ships inside your \
                     application, which is why it cannot send. Mint one at \
                     Settings ▸ Tokens with scope `api`.",
        })),
    ))
}
