//! Push credentials admin endpoints (vendor secrets for APNs /
//! FCM / WebPush / HCM / MiPush).
//!
//! - `POST   /admin/api/projects/:project_id/push/credentials` — upsert
//! - `GET    /admin/api/projects/:project_id/push/credentials` — list
//! - `DELETE /admin/api/projects/:project_id/push/credentials/:kind`
//!
//! Without these credentials, the push_send queue cannot deliver.
//! Phase D step 5+ wires the dispatcher worker that reads from
//! push_credentials before calling vendor APIs.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Extension, Path, State},
    http::{HeaderMap, StatusCode},
};

use crate::handlers::tenant::guard_project;
use crate::session_mw::SessionContext;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use tracing::{info, warn};
use uuid::Uuid;

use crate::state::AppState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertBody {
    /// Provider: `apns` / `fcm` / `webpush` / `hcm` / `mipush`.
    pub provider: String,
    /// Free-form vendor config (APNs key id + team id, FCM
    /// service-account json, WebPush vapid keys, etc.). Stored
    /// as JSONB.
    pub config: Value,
    /// Secret material — encrypted at rest in a follow-up commit;
    /// for v0.2 step 1 stored as bytea verbatim.
    pub secret: Option<String>,
}

pub async fn upsert(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
    headers: HeaderMap,
    Json(body): Json<UpsertBody>,
) -> (StatusCode, Json<Value>) {
    if !matches!(
        body.provider.as_str(),
        "apns" | "fcm" | "webpush" | "hcm" | "mipush"
    ) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "invalid_provider" })),
        );
    }

    // Tenant guard: the project must belong to the caller's
    // workspace. The INSERT derives workspace_id from the project
    // row, so without this a caller could plant credentials on
    // another tenant's project.
    if let Err((code, msg)) = guard_project(&state, ctx.workspace_id, project_id).await {
        return (code, Json(json!({ "error": msg })));
    }

    // Use device_tokens-side push_credentials table from migration 0024.
    // workspace_id derived via projects FK subquery (matches pattern in
    // migrations 0016+).
    let id = Uuid::now_v7();
    let secret_bytes = body.secret.unwrap_or_default().into_bytes();
    let result = sqlx::query(
        "INSERT INTO push_credentials \
         (id, workspace_id, project_id, kind, config, secret_blob) \
         SELECT $1, p.workspace_id, $2, $3, $4, $5 FROM projects p WHERE p.id = $2 \
         ON CONFLICT (project_id, kind) DO UPDATE SET \
            config = EXCLUDED.config, \
            secret_blob = EXCLUDED.secret_blob, \
            last_validated_at = NULL, \
            last_validate_status = NULL \
         RETURNING id",
    )
    .bind(id)
    .bind(project_id)
    .bind(&body.provider)
    .bind(&body.config)
    .bind(&secret_bytes)
    .fetch_optional(&state.pool)
    .await;

    match result {
        Ok(Some(row)) => {
            let id: Uuid = row.get("id");
            info!(
                %project_id,
                provider = %body.provider,
                "admin.push_credentials upserted",
            );
            let (ip, ua) = crate::notify::extract_request_meta(&headers);
            crate::notify::audit(
                &state.pool,
                ctx.workspace_id.into_uuid(),
                Some(project_id),
                Some(ctx.user_id.into_uuid()),
                "push_credentials.upsert",
                Some("push_credentials"),
                Some(&id.to_string()),
                crate::notify::enrich_payload(
                    json!({ "provider": body.provider }),
                    ip.as_deref(),
                    ua.as_deref(),
                ),
            )
            .await;
            (
                StatusCode::CREATED,
                Json(json!({ "id": id.to_string(), "provider": body.provider })),
            )
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "project_not_found" })),
        ),
        Err(e) => {
            warn!(error = %e, "admin.push_credentials upsert_failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
) -> Json<Value> {
    if guard_project(&state, ctx.workspace_id, project_id)
        .await
        .is_err()
    {
        return Json(json!({ "credentials": [] }));
    }
    let rows = sqlx::query(
        "SELECT id, kind, config, created_at, last_validated_at, last_validate_status \
         FROM push_credentials WHERE project_id = $1 ORDER BY kind",
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
                "created_at": crate::wire_time::rfc3339(r.get::<time::OffsetDateTime, _>("created_at")),
                "last_validated_at": crate::wire_time::rfc3339_opt(r.get::<Option<time::OffsetDateTime>, _>("last_validated_at")),
                "last_validate_status": r.get::<Option<String>, _>("last_validate_status"),
            })
        })
        .collect();
    Json(json!({ "credentials": out }))
}

pub async fn delete(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path((project_id, kind)): Path<(Uuid, String)>,
) -> StatusCode {
    if let Err((code, _)) = guard_project(&state, ctx.workspace_id, project_id).await {
        return code;
    }
    let result = sqlx::query("DELETE FROM push_credentials WHERE project_id = $1 AND kind = $2")
        .bind(project_id)
        .bind(&kind)
        .execute(&state.pool)
        .await;
    match result {
        Ok(_) => StatusCode::NO_CONTENT,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
