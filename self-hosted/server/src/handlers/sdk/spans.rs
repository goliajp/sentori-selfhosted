//! POST `/v1/spans` — single distributed trace span.

use std::sync::Arc;

use axum::{Extension, Json, extract::State, http::StatusCode};
use sentori_billing::CounterKind;
use sentori_ingest_token::IngestContext;
use sentori_span_store::{SpanInput, SpanStoreError};
use serde_json::{Value, json};
use time::OffsetDateTime;
use tracing::{info, warn};

use crate::handlers::sdk::quota;
use crate::state::AppState;

pub async fn handle(
    Extension(ctx): Extension<IngestContext>,
    State(state): State<Arc<AppState>>,
    Json(input): Json<SpanInput>,
) -> (StatusCode, Json<Value>) {
    // K17 quota: meter one span against the project's plan.
    let now = OffsetDateTime::now_utc();
    if let Err(body) = quota::meter(&state, ctx.project_id, CounterKind::Spans, 1, now).await {
        return (StatusCode::PAYMENT_REQUIRED, Json(body));
    }

    match state.spans.ingest_span(ctx.project_id, input).await {
        Ok(span) => {
            info!(
                workspace_id = %ctx.workspace_id,
                project_id = %ctx.project_id,
                span_id = %span.id,
                trace_id = %span.trace_id,
                "sdk.spans ingested",
            );
            (
                StatusCode::ACCEPTED,
                Json(json!({
                    "span_id": span.id.to_string(),
                    "trace_id": span.trace_id.to_string(),
                })),
            )
        }
        Err(SpanStoreError::ProjectNotFound(_)) => (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "project_not_found" })),
        ),
        Err(SpanStoreError::InvalidSpan(msg)) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "invalid_span", "detail": msg })),
        ),
        Err(e) => {
            warn!(workspace_id = %ctx.workspace_id, error = %e, "sdk.spans db_error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}
