//! GET /admin/api/audit — audit log query (superadmin only).

use std::sync::Arc;

use axum::{Extension, Json, extract::Query, extract::State, http::StatusCode};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;

use crate::session_mw::SessionContext;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct AuditQuery {
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub project_id: Option<uuid::Uuid>,
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Query(q): Query<AuditQuery>,
) -> (StatusCode, Json<Value>) {
    if !ctx.role.is_superadmin() {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "superadmin_only" })),
        );
    }
    let limit = q.limit.unwrap_or(100).clamp(1, 500);
    let rows = sqlx::query(
        "SELECT a.id, a.project_id, a.actor_user_id, u.email AS actor_email, \
                a.action, a.target_type, a.target_id, a.payload, a.created_at \
         FROM audit_logs a LEFT JOIN users u ON u.id = a.actor_user_id \
         WHERE ($2::uuid IS NULL OR a.project_id = $2) \
         ORDER BY a.created_at DESC LIMIT $1",
    )
    .bind(limit)
    .bind(q.project_id)
    .fetch_all(&state.pool)
    .await;
    match rows {
        Ok(rows) => {
            let out: Vec<Value> = rows
                .iter()
                .map(|r| {
                    json!({
                        "id": r.get::<uuid::Uuid, _>("id"),
                        "projectId": r.get::<Option<uuid::Uuid>, _>("project_id"),
                        "actorUserId": r.get::<Option<uuid::Uuid>, _>("actor_user_id"),
                        "actorEmail": r.get::<Option<String>, _>("actor_email"),
                        "action": r.get::<String, _>("action"),
                        "targetType": r.get::<Option<String>, _>("target_type"),
                        "targetId": r.get::<Option<String>, _>("target_id"),
                        "payload": r.get::<Value, _>("payload"),
                        "createdAt": crate::wire_time::rfc3339(r.get("created_at")),
                    })
                })
                .collect();
            (StatusCode::OK, Json(json!({ "entries": out })))
        }
        Err(e) => {
            tracing::warn!(error = %e, "audit query failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}
