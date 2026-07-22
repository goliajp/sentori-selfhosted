//! POST `/v1/metrics:batch` + `/v1/runtime-metrics:batch` —
//! custom + auto-instrumented timeseries.
//!
//! Both endpoints hit the same `MetricsStore::ingest_batch`
//! function. The MetricPoint shape includes a per-point
//! `projectId`; we override it with the auth-bound project_id
//! so SDK can't forge cross-project writes.

use std::sync::Arc;

use axum::{Extension, Json, extract::State, http::StatusCode};
use sentori_ingest_token::IngestContext;
use sentori_runtime_metrics::MetricPoint;
use serde_json::{Value, json};
use tracing::{info, warn};

use crate::state::AppState;

const MAX_BATCH_SIZE: usize = 500;

pub async fn handle(
    Extension(ctx): Extension<IngestContext>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let arr = if let Some(a) = payload.as_array() {
        a.clone()
    } else if let Some(a) = payload.get("points").and_then(|v| v.as_array()) {
        a.clone()
    } else if let Some(a) = payload.get("metrics").and_then(|v| v.as_array()) {
        a.clone()
    } else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "expected array or { points: [...] }" })),
        );
    };

    if arr.len() > MAX_BATCH_SIZE {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "batch too large",
                "max": MAX_BATCH_SIZE,
                "got": arr.len(),
            })),
        );
    }

    // Deserialize each point + override project_id with the
    // auth-bound one (SDK can't cross-project write).
    let mut points: Vec<MetricPoint> = Vec::with_capacity(arr.len());
    let mut bad = 0u32;
    for raw in arr {
        match serde_json::from_value::<MetricPoint>(raw) {
            Ok(mut p) => {
                p.project_id = ctx.project_id;
                points.push(p);
            }
            Err(_) => bad += 1,
        }
    }

    match state.metrics.ingest_batch(&points).await {
        Ok(written) => {
            info!(
                workspace_id = %ctx.workspace_id,
                project_id = %ctx.project_id,
                accepted = written,
                bad,
                "sdk.metrics ingested",
            );
            (
                StatusCode::ACCEPTED,
                Json(json!({ "accepted": written, "failed": bad })),
            )
        }
        Err(e) => {
            warn!(workspace_id = %ctx.workspace_id, error = %e, "sdk.metrics db_error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}
