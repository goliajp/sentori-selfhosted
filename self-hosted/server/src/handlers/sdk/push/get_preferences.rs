//! GET `/v1/push/users/{user_key}/preferences`

use std::sync::Arc;

use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};
use sentori_ingest_token::IngestContext;
use serde_json::{Value, json};
use sqlx::Row;
use tracing::warn;

use crate::state::AppState;

pub async fn handle(
    Extension(ctx): Extension<IngestContext>,
    State(state): State<Arc<AppState>>,
    Path(user_key): Path<String>,
) -> (StatusCode, Json<Value>) {
    // Both of these answered 200. The sibling PUT on the same key
    // shape answers 400, so a client saw the same mistake two ways.
    let fp_bytes = match hex::decode(&user_key) {
        Ok(b) if b.len() == 32 => b,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "invalid_fp_hex" })),
            );
        }
    };

    let rows = match sqlx::query(
        "SELECT category, opted_out, updated_at FROM push_preferences \
         WHERE project_id = $1 AND user_fingerprint_hex = $2 \
         ORDER BY category",
    )
    .bind(ctx.project_id)
    .bind(&fp_bytes)
    .fetch_all(&state.pool)
    .await
    {
        Ok(rs) => rs,
        Err(e) => {
            warn!(error = %e, "push.get_preferences db_error");
            // Emphatically not `200 {"preferences": []}`. A caller
            // reading that field sees "this person has opted out of
            // nothing" where the truth is "we do not know" — and then
            // sends to somebody who may have opted out.
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            );
        }
    };

    let prefs: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "category": r.get::<String, _>("category"),
                "optedOut": r.get::<bool, _>("opted_out"),
                "updatedAt": crate::wire_time::rfc3339(r.get::<time::OffsetDateTime, _>("updated_at")),
            })
        })
        .collect();

    (StatusCode::OK, Json(json!({ "preferences": prefs })))
}
