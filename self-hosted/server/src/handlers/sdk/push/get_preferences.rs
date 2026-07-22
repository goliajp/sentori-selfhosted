//! GET `/v1/push/users/{fp_hex}/preferences`

use std::sync::Arc;

use axum::{
    Extension, Json,
    extract::{Path, State},
};
use sentori_ingest_token::IngestContext;
use serde_json::{Value, json};
use sqlx::Row;
use tracing::warn;

use crate::state::AppState;

pub async fn handle(
    Extension(ctx): Extension<IngestContext>,
    State(state): State<Arc<AppState>>,
    Path(fp_hex): Path<String>,
) -> Json<Value> {
    let fp_bytes = match hex::decode(&fp_hex) {
        Ok(b) if b.len() == 32 => b,
        _ => return Json(json!({ "error": "invalid_fp_hex" })),
    };

    let rows = match sqlx::query(
        "SELECT category, opted_out, updated_at FROM push_preferences \
         WHERE project_id = $1 AND user_fingerprint_hex = $2 \
         ORDER BY category",
    )
    .bind(ctx.project_id.into_uuid())
    .bind(&fp_bytes)
    .fetch_all(&state.pool)
    .await
    {
        Ok(rs) => rs,
        Err(e) => {
            warn!(error = %e, "push.get_preferences db_error");
            return Json(json!({ "preferences": [], "error": "internal" }));
        }
    };

    let prefs: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "category": r.get::<String, _>("category"),
                "opted_out": r.get::<bool, _>("opted_out"),
                "updated_at": crate::wire_time::rfc3339(r.get::<time::OffsetDateTime, _>("updated_at")),
            })
        })
        .collect();

    Json(json!({ "preferences": prefs }))
}
