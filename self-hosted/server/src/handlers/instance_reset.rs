//! A recent owner password and typed identity authorize a short offline window.
use std::sync::Arc;

use axum::{Extension, Json, extract::State, http::StatusCode};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::instance_operations::{Reply, fail, internal, owner};
use crate::{session_mw::SessionContext, state::AppState};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Prepare {
    instance_id: Uuid,
    generation: i64,
    instance_name: String,
    current_password: String,
}

pub async fn prepare(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Json(body): Json<Prepare>,
) -> Reply {
    owner(&ctx)?;
    if !crate::instance_operations::configured(&state) {
        return Err(fail(StatusCode::CONFLICT, "backup_not_configured"));
    }
    if body.current_password.len() > 1024 || body.instance_name.len() > 320 {
        return Err(fail(StatusCode::BAD_REQUEST, "invalid_reset_request"));
    }
    let mut tx = state.pool.begin().await.map_err(internal)?;
    // Same user-first lock order as password rotation, then the instance.
    let user: Option<(String, String)> =
        sqlx::query_as("SELECT password_hash,role FROM users WHERE id=$1 FOR UPDATE")
            .bind(ctx.user_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(internal)?;
    let Some((password_hash, role)) = user else {
        return Err(fail(StatusCode::FORBIDDEN, "owner_required"));
    };
    let live: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM auth_sessions WHERE id_hash=$1 AND user_id=$2 AND expires_at>now())")
        .bind(ctx.session_id_hash.as_slice()).bind(ctx.user_id).fetch_one(&mut *tx).await.map_err(internal)?;
    if role != "superadmin" || !live {
        return Err(fail(StatusCode::FORBIDDEN, "owner_required"));
    }
    if !sentori_argon2_password::PasswordHash::verify(&body.current_password, &password_hash)
        .unwrap_or(false)
    {
        return Err(fail(StatusCode::BAD_REQUEST, "wrong_password"));
    }
    let (id, name, generation, mode, pending): (Uuid, String, i64, String, bool) = sqlx::query_as("SELECT id,name,generation,operating_mode,reset_pending IS NOT NULL FROM instance_settings WHERE singleton FOR UPDATE")
        .fetch_one(&mut *tx).await.map_err(internal)?;
    if id != body.instance_id
        || name != body.instance_name
        || generation != body.generation
        || mode != "normal"
        || pending
    {
        return Err(fail(StatusCode::CONFLICT, "instance_changed"));
    }
    let active: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM instance_operations WHERE state IN ('queued','running'))",
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(internal)?;
    if active {
        return Err(fail(StatusCode::CONFLICT, "backup_in_progress"));
    }
    let operation = Uuid::now_v7();
    let permit = sentori_snapshot::RecoveryKey::generate().map_err(internal)?;
    let permit = hex::encode(permit.as_bytes());
    // Only the latest permit from this owner remains valid.
    sqlx::query("DELETE FROM instance_reset_intents WHERE actor_user_id=$1 OR expires_at<=now()")
        .bind(ctx.user_id)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    let expires: time::OffsetDateTime = sqlx::query_scalar("INSERT INTO instance_reset_intents(id,instance_id,generation,instance_name,actor_user_id,session_hash,password_fingerprint,permit_hash) VALUES($1,$2,$3,$4,$5,$6,$7,$8) RETURNING expires_at")
        .bind(operation).bind(id).bind(generation).bind(&name).bind(ctx.user_id).bind(ctx.session_id_hash.as_slice())
        .bind(hex::encode(Sha256::digest(password_hash.as_bytes()))).bind(hex::encode(Sha256::digest(permit.as_bytes())))
        .fetch_one(&mut *tx).await.map_err(internal)?;
    sqlx::query("INSERT INTO audit_logs(id,actor_user_id,action,target_type,target_id,payload) VALUES($1,$2,'instance.reset.prepared','instance',$3,$4)")
        .bind(Uuid::now_v7()).bind(ctx.user_id).bind(id.to_string()).bind(json!({"operationId":operation,"generation":generation,"expiresAt":crate::wire_time::rfc3339(expires)}))
        .execute(&mut *tx).await.map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok(Json(
        json!({"operationId":operation,"permit":permit,"expiresAt":crate::wire_time::rfc3339(expires)}),
    ))
}
