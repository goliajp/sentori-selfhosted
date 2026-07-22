//! Integration admin endpoints — external service connections
//! (Slack, Linear, Jira, GitHub, GitLab webhook configs).
//!
//! - `GET    /admin/api/projects/:project_id/integrations` — list
//! - `POST   /admin/api/projects/:project_id/integrations` — upsert
//! - `DELETE /admin/api/projects/:project_id/integrations/:kind` — drop
//! - `PATCH  /admin/api/projects/:project_id/integrations/:kind/active` — flip

use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use tracing::{info, warn};
use uuid::Uuid;

use crate::state::AppState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertBody {
    pub kind: String,
    pub config: Value,
    #[serde(default)]
    pub connected_by: Option<Uuid>,
}

pub async fn upsert(
    State(state): State<Arc<AppState>>,
    Path(project_id): Path<Uuid>,
    Json(body): Json<UpsertBody>,
) -> (StatusCode, Json<Value>) {
    if body.kind.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "kind required" })),
        );
    }
    let id = Uuid::now_v7();
    let result = sqlx::query(
        "INSERT INTO integrations (id, workspace_id, project_id, kind, config, connected_by, active) \
         SELECT $1, p.workspace_id, $2, $3, $4, $5, TRUE FROM projects p WHERE p.id = $2 \
         ON CONFLICT (project_id, kind) DO UPDATE SET \
            config = EXCLUDED.config, \
            connected_by = COALESCE(EXCLUDED.connected_by, integrations.connected_by), \
            active = TRUE \
         RETURNING id",
    )
    .bind(id)
    .bind(project_id)
    .bind(&body.kind)
    .bind(&body.config)
    .bind(body.connected_by)
    .fetch_optional(&state.pool)
    .await;

    match result {
        Ok(Some(row)) => {
            let id: Uuid = row.get("id");
            info!(
                %project_id,
                kind = %body.kind,
                "admin.integrations upserted",
            );
            (
                StatusCode::CREATED,
                Json(json!({ "id": id.to_string(), "kind": body.kind })),
            )
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "project_not_found" })),
        ),
        Err(e) => {
            warn!(error = %e, "admin.integrations upsert_failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}

pub async fn list(State(state): State<Arc<AppState>>, Path(project_id): Path<Uuid>) -> Json<Value> {
    let rows = sqlx::query(
        "SELECT id, kind, config, connected_by, connected_at, active \
         FROM integrations WHERE project_id = $1 ORDER BY kind",
    )
    .bind(project_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let out: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<Uuid, _>("id").to_string(),
                "kind": r.get::<String, _>("kind"),
                "config": r.get::<Value, _>("config"),
                "connected_by": r.try_get::<Option<Uuid>, _>("connected_by").ok().flatten().map(|u| u.to_string()),
                "connected_at": crate::wire_time::rfc3339(r.get::<time::OffsetDateTime, _>("connected_at")),
                "active": r.get::<bool, _>("active"),
            })
        })
        .collect();
    Json(json!({ "integrations": out }))
}

pub async fn delete(
    State(state): State<Arc<AppState>>,
    Path((project_id, kind)): Path<(Uuid, String)>,
) -> StatusCode {
    match sqlx::query("DELETE FROM integrations WHERE project_id = $1 AND kind = $2")
        .bind(project_id)
        .bind(&kind)
        .execute(&state.pool)
        .await
    {
        Ok(_) => StatusCode::NO_CONTENT,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveBody {
    pub active: bool,
}

pub async fn set_active(
    State(state): State<Arc<AppState>>,
    Path((project_id, kind)): Path<(Uuid, String)>,
    Json(body): Json<ActiveBody>,
) -> StatusCode {
    match sqlx::query("UPDATE integrations SET active = $1 WHERE project_id = $2 AND kind = $3")
        .bind(body.active)
        .bind(project_id)
        .bind(&kind)
        .execute(&state.pool)
        .await
    {
        Ok(_) => StatusCode::NO_CONTENT,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
