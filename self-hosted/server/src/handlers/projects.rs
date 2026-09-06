//! GET /admin/api/projects — the caller's visible project list.

use std::sync::Arc;

use axum::{Extension, Json, extract::State, http::StatusCode};
use serde_json::{Value, json};
use sqlx::Row;
use tracing::warn;
use uuid::Uuid;

use crate::session_mw::SessionContext;
use crate::state::AppState;

pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
) -> (StatusCode, Json<Value>) {
    let rows = if ctx.role.is_superadmin() {
        sqlx::query("SELECT id, name, platform, created_at FROM projects ORDER BY created_at")
            .fetch_all(&state.pool)
            .await
    } else {
        sqlx::query(
            "SELECT p.id, p.name, p.platform, p.created_at FROM projects p \
             JOIN project_assignments pa ON pa.project_id = p.id \
             WHERE pa.user_id = $1 ORDER BY p.created_at",
        )
        .bind(ctx.user_id)
        .fetch_all(&state.pool)
        .await
    };
    match rows {
        Ok(rows) => {
            let out: Vec<Value> = rows
                .iter()
                .map(|r| {
                    json!({
                        "id": r.get::<Uuid, _>("id"),
                        "name": r.get::<String, _>("name"),
                        "platform": r.get::<String, _>("platform"),
                        "createdAt": crate::wire_time::rfc3339(r.get("created_at")),
                    })
                })
                .collect();
            (StatusCode::OK, Json(json!({ "projects": out })))
        }
        Err(e) => {
            warn!(error = %e, "project list failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}
