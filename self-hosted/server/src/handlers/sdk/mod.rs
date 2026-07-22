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

pub mod control;
pub mod deploys;
pub mod events;
pub mod events_attachments;
pub mod events_batch;
pub mod events_recent;
pub mod heartbeat;
pub mod metrics;
pub mod push;
pub mod quota;
pub mod runtime_metrics;
pub mod security_link;
pub mod security_report;
pub mod security_score;
pub mod sessions;
pub mod spans;
pub mod spans_batch;
pub mod track;
pub mod user_reports;

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
    if ctx.token_kind.is_admin() {
        return Ok(());
    }
    Err((
        axum::http::StatusCode::FORBIDDEN,
        axum::Json(serde_json::json!({
            "error": "admin_token_required",
            "hint": "this endpoint needs a token of kind `admin`; the token used is `public` and is shipped inside your application",
        })),
    ))
}
