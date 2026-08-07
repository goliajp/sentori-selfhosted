//! Admin account management (superadmin only) — design.md §9.
//!
//! The owner creates admins with an initial password (no invite
//! email flow — hand it over in person or let them reset via
//! SMTP). Assignments are the entire permission model: an admin
//! sees exactly the projects listed here.

use std::sync::Arc;

use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use tracing::warn;
use uuid::Uuid;

use crate::session_mw::SessionContext;
use crate::state::AppState;

fn superadmin_only(ctx: &SessionContext) -> Result<(), (StatusCode, Json<Value>)> {
    if ctx.role.is_superadmin() {
        Ok(())
    } else {
        Err((
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "superadmin_only" })),
        ))
    }
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
) -> (StatusCode, Json<Value>) {
    if let Err(e) = superadmin_only(&ctx) {
        return e;
    }
    let rows = sqlx::query(
        "SELECT u.id, u.email, u.role, u.display_name, u.created_at, u.last_login_at, \
                COALESCE(array_agg(pa.project_id) FILTER (WHERE pa.project_id IS NOT NULL), '{}') AS projects \
         FROM users u LEFT JOIN project_assignments pa ON pa.user_id = u.id \
         GROUP BY u.id ORDER BY u.created_at",
    )
    .fetch_all(&state.pool)
    .await;
    match rows {
        Ok(rows) => {
            let out: Vec<Value> = rows
                .iter()
                .map(|r| {
                    json!({
                        "id": r.get::<Uuid, _>("id"),
                        "email": r.get::<String, _>("email"),
                        "role": r.get::<String, _>("role"),
                        "displayName": r.get::<Option<String>, _>("display_name"),
                        "createdAt": crate::wire_time::rfc3339(r.get("created_at")),
                        "lastLoginAt": crate::wire_time::rfc3339_opt(r.get("last_login_at")),
                        "projects": r.get::<Vec<Uuid>, _>("projects"),
                    })
                })
                .collect();
            (StatusCode::OK, Json(json!({ "users": out })))
        }
        Err(e) => {
            warn!(error = %e, "user list failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBody {
    pub email: String,
    pub password: String,
    #[serde(default)]
    pub display_name: Option<String>,
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Json(body): Json<CreateBody>,
) -> (StatusCode, Json<Value>) {
    if let Err(e) = superadmin_only(&ctx) {
        return e;
    }
    if body.password.len() < 8 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "password_too_short", "min": 8 })),
        );
    }
    let Ok(phc) = sentori_argon2_password::PasswordHash::hash(&body.password) else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "hash_failed" })),
        );
    };
    let id = Uuid::now_v7();
    let r = sqlx::query(
        "INSERT INTO users (id, email, password_hash, role, display_name) \
         VALUES ($1, $2, $3, 'admin', $4)",
    )
    .bind(id)
    .bind(&body.email)
    .bind(&phc)
    .bind(body.display_name.as_deref())
    .execute(&state.pool)
    .await;
    match r {
        Ok(_) => {
            crate::audit::record(
                &state.pool,
                None,
                ctx.user_id,
                "user.create",
                "user",
                &id.to_string(),
                json!({ "email": body.email }),
            )
            .await;
            (StatusCode::CREATED, Json(json!({ "id": id })))
        }
        Err(e)
            if e.as_database_error()
                .is_some_and(sqlx::error::DatabaseError::is_unique_violation) =>
        {
            (
                StatusCode::CONFLICT,
                Json(json!({ "error": "email_taken" })),
            )
        }
        Err(e) => {
            warn!(error = %e, "user create failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}

pub async fn delete(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(user_id): Path<Uuid>,
) -> (StatusCode, Json<Value>) {
    if let Err(e) = superadmin_only(&ctx) {
        return e;
    }
    if user_id == ctx.user_id {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "cannot_delete_self" })),
        );
    }
    let r = sqlx::query("DELETE FROM users WHERE id = $1 AND role = 'admin'")
        .bind(user_id)
        .execute(&state.pool)
        .await;
    match r {
        Ok(res) if res.rows_affected() > 0 => {
            crate::audit::record(
                &state.pool,
                None,
                ctx.user_id,
                "user.delete",
                "user",
                &user_id.to_string(),
                json!({}),
            )
            .await;
            (StatusCode::OK, Json(json!({ "ok": true })))
        }
        Ok(_) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "admin_not_found" })),
        ),
        Err(e) => {
            warn!(error = %e, "user delete failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}

/// PUT /admin/api/users/{user_id}/projects/{project_id} — assign.
pub async fn assign(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path((user_id, project_id)): Path<(Uuid, Uuid)>,
) -> (StatusCode, Json<Value>) {
    if let Err(e) = superadmin_only(&ctx) {
        return e;
    }
    let r = sqlx::query(
        "INSERT INTO project_assignments (user_id, project_id, assigned_by) \
         VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(user_id)
    .bind(project_id)
    .bind(ctx.user_id)
    .execute(&state.pool)
    .await;
    match r {
        Ok(_) => {
            crate::audit::record(
                &state.pool,
                Some(project_id),
                ctx.user_id,
                "assignment.grant",
                "user",
                &user_id.to_string(),
                json!({}),
            )
            .await;
            (StatusCode::OK, Json(json!({ "ok": true })))
        }
        Err(e) => {
            warn!(error = %e, "assign failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}

/// DELETE /admin/api/users/{user_id}/projects/{project_id} — revoke.
pub async fn unassign(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path((user_id, project_id)): Path<(Uuid, Uuid)>,
) -> (StatusCode, Json<Value>) {
    if let Err(e) = superadmin_only(&ctx) {
        return e;
    }
    let r = sqlx::query("DELETE FROM project_assignments WHERE user_id = $1 AND project_id = $2")
        .bind(user_id)
        .bind(project_id)
        .execute(&state.pool)
        .await;
    match r {
        Ok(_) => {
            crate::audit::record(
                &state.pool,
                Some(project_id),
                ctx.user_id,
                "assignment.revoke",
                "user",
                &user_id.to_string(),
                json!({}),
            )
            .await;
            (StatusCode::OK, Json(json!({ "ok": true })))
        }
        Err(e) => {
            warn!(error = %e, "unassign failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}
