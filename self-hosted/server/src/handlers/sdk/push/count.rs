//! POST `/v1/push/count` — how many devices an audience selects.
//!
//! The console has counted before sending since audiences existed, and
//! refuses to send to a number nobody read. A backend had neither: the
//! preview lives on the admin surface behind a browser session, so the
//! one caller that can send to a condition automatically was also the
//! one that could not find out how large it was first.
//!
//! That asymmetry is the wrong way round. A person clicking a button
//! is watching; a nightly job is not, and a condition that quietly
//! grew from four hundred to forty thousand looks exactly the same to
//! it either way.
//!
//! The same compiled query the send runs, with `count(*)` in front, so
//! this is not an estimate of what a send would do — it is the same
//! question asked without the consequence.

use std::sync::Arc;

use axum::{Extension, Json, extract::State, http::StatusCode};
use sentori_ingest_token::IngestContext;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{AssertSqlSafe, Row};
use tracing::warn;

use crate::state::AppState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CountBody {
    #[serde(default)]
    pub app_user_id: Option<String>,
    #[serde(default)]
    pub traits: Option<Value>,
    #[serde(default)]
    pub audience: Option<Value>,
}

pub async fn handle(
    Extension(ctx): Extension<IngestContext>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<CountBody>,
) -> (StatusCode, Json<Value>) {
    // The same scope the send needs. Counting a customer's users is
    // not something the token inside their shipped app should do.
    if let Err((code, body)) = crate::handlers::sdk::require_admin_token(&ctx) {
        return (code, body);
    }

    let audience = match crate::audience::from_request(
        body.app_user_id.as_deref(),
        body.traits.as_ref(),
        body.audience.as_ref(),
    ) {
        Ok(Some(a)) => a,
        Ok(None) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "bad_target",
                    "detail": "give appUserId, traits or audience. Device-shaped \
                               targets (spTokens, nativeTokens, topic) are lists \
                               you already hold the length of.",
                })),
            );
        }
        Err(detail) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "bad_target", "detail": detail })),
            );
        }
    };

    let (frag, binds) = audience.to_sql(2);
    let sql = format!(
        "SELECT count(*) AS n FROM device_tokens dt \
         WHERE dt.project_id = $1 AND dt.revoked_at IS NULL AND ({frag})"
    );
    // Audited for injection: the selector is `audience.rs` output —
    // `$n` placeholders and enum-supplied column names only. Operator
    // values are binds.
    let mut q = sqlx::query(AssertSqlSafe(sql.clone())).bind(ctx.project_id);
    for b in &binds {
        q = b.attach(q);
    }

    match q.fetch_one(&state.pool).await {
        Ok(r) => (
            StatusCode::OK,
            Json(json!({ "matched": r.get::<i64, _>("n") })),
        ),
        Err(e) => {
            // The compiler produces valid SQL for anything it
            // accepted, so a failure here is ours. Answering zero
            // would read as an answer, and a caller deciding whether
            // to send would act on it.
            warn!(error = %e, "push.count failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}
