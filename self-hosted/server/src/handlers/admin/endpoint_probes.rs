//! Endpoint check admin CRUD — synthetic HTTP monitor.
//! Schema per migration 0029 (endpoint_check + endpoint_probe).

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
pub struct CreateBody {
    pub name: String,
    pub target_url: String,
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub interval_sec: Option<i32>,
    #[serde(default)]
    pub assertion_status_codes: Option<Vec<i32>>,
    #[serde(default)]
    pub assertion_max_latency_ms: Option<i32>,
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreateBody>,
) -> (StatusCode, Json<Value>) {
    if body.target_url.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "target_url required" })),
        );
    }
    let id = Uuid::now_v7();
    let interval = body.interval_sec.unwrap_or(60).max(60);
    let codes = body.assertion_status_codes.unwrap_or_else(|| vec![200]);
    let result = sqlx::query(
        "INSERT INTO endpoint_check (id, workspace_id, project_id, name, target_url, method, \
            interval_sec, assertion_status_codes, assertion_max_latency_ms) \
         SELECT $1, p.workspace_id, $2, $3, $4, $5, $6, $7, $8 \
         FROM projects p WHERE p.id = $2 \
         RETURNING id",
    )
    .bind(id)
    .bind(project_id)
    .bind(&body.name)
    .bind(&body.target_url)
    .bind(body.method.as_deref().unwrap_or("GET"))
    .bind(interval)
    .bind(&codes)
    .bind(body.assertion_max_latency_ms)
    .fetch_optional(&state.pool)
    .await;
    match result {
        Ok(Some(row)) => {
            let id: Uuid = row.get("id");
            info!(%project_id, url = %body.target_url, "admin.endpoint_check created");
            (StatusCode::CREATED, Json(json!({ "id": id.to_string() })))
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "project_not_found" })),
        ),
        Err(e) => {
            warn!(error = %e, "admin.endpoint_check create_failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}

pub async fn list(State(state): State<Arc<AppState>>, Path(project_id): Path<Uuid>) -> Json<Value> {
    let rows = sqlx::query(
        "SELECT id, name, target_url, method, interval_sec, assertion_status_codes, \
                assertion_max_latency_ms, paused, created_at \
         FROM endpoint_check WHERE project_id = $1 ORDER BY created_at DESC",
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
                "name": r.get::<String, _>("name"),
                "endpoint_url": r.get::<String, _>("target_url"),
                "method": r.get::<String, _>("method"),
                "expected_status": r
                    .try_get::<Vec<i32>, _>("assertion_status_codes")
                    .unwrap_or_default()
                    .first()
                    .copied()
                    .unwrap_or(200),
                "interval_sec": r.get::<i32, _>("interval_sec"),
                "timeout_ms": r.try_get::<Option<i32>, _>("assertion_max_latency_ms").ok().flatten().unwrap_or(5000),
                "enabled": !r.get::<bool, _>("paused"),
                "created_at": crate::wire_time::rfc3339(r.get::<time::OffsetDateTime, _>("created_at")),
            })
        })
        .collect();
    Json(json!({ "probes": out }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchBody {
    pub enabled: Option<bool>,
}

pub async fn patch(
    State(state): State<Arc<AppState>>,
    Path(check_id): Path<Uuid>,
    Json(body): Json<PatchBody>,
) -> StatusCode {
    if let Some(en) = body.enabled {
        let _ = sqlx::query("UPDATE endpoint_check SET paused = $1 WHERE id = $2")
            .bind(!en)
            .bind(check_id)
            .execute(&state.pool)
            .await;
    }
    StatusCode::NO_CONTENT
}

pub async fn delete(State(state): State<Arc<AppState>>, Path(check_id): Path<Uuid>) -> StatusCode {
    let _ = sqlx::query("DELETE FROM endpoint_check WHERE id = $1")
        .bind(check_id)
        .execute(&state.pool)
        .await;
    StatusCode::NO_CONTENT
}
