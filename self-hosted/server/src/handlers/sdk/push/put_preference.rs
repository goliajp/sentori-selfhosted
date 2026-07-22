//! PUT `/v1/push/users/{fp_hex}/preferences/{category}`

use std::sync::Arc;

use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};
use sentori_ingest_token::IngestContext;
use serde::Deserialize;
use serde_json::{Value, json};
use tracing::{info, warn};

use crate::state::AppState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreferenceBody {
    pub opted_out: bool,
}

pub async fn handle(
    Extension(ctx): Extension<IngestContext>,
    State(state): State<Arc<AppState>>,
    Path((fp_hex, category)): Path<(String, String)>,
    Json(body): Json<PreferenceBody>,
) -> (StatusCode, Json<Value>) {
    let fp_bytes = match hex::decode(&fp_hex) {
        Ok(b) if b.len() == 32 => b,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "invalid_fp_hex" })),
            );
        }
    };
    if category.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "category required" })),
        );
    }

    let result = sqlx::query(
        "INSERT INTO push_preferences (project_id, user_fingerprint_hex, category, opted_out) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (project_id, user_fingerprint_hex, category) DO UPDATE SET \
            opted_out = EXCLUDED.opted_out, \
            updated_at = now()",
    )
    .bind(ctx.project_id.into_uuid())
    .bind(&fp_bytes)
    .bind(&category)
    .bind(body.opted_out)
    .execute(&state.pool)
    .await;

    match result {
        Ok(_) => {
            info!(
                workspace_id = %ctx.workspace_id,
                project_id = %ctx.project_id,
                %category,
                opted_out = body.opted_out,
                "push.put_preference upserted",
            );
            (
                StatusCode::ACCEPTED,
                Json(
                    json!({ "status": "updated", "category": category, "opted_out": body.opted_out }),
                ),
            )
        }
        Err(e) => {
            warn!(error = %e, "push.put_preference db_error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}
