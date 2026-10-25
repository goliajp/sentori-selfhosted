//! Owner-authorized backup commands and streamed encrypted downloads.
use crate::{instance_operations as operations, session_mw::SessionContext, state::AppState};
use axum::{
    Extension, Json,
    body::Body,
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use uuid::Uuid;
pub(super) type Error = (StatusCode, Json<Value>);
pub(super) type Reply = Result<Json<Value>, Error>;
pub(super) fn fail(status: StatusCode, code: &str) -> Error {
    (status, Json(json!({"error":code,"code":code})))
}
pub(super) fn internal(error: impl std::fmt::Display) -> Error {
    tracing::warn!(%error,"instance operation request failed");
    fail(StatusCode::INTERNAL_SERVER_ERROR, "internal")
}
pub(super) fn owner(ctx: &SessionContext) -> Result<(), Error> {
    if ctx.role.is_superadmin() {
        Ok(())
    } else {
        Err(fail(StatusCode::FORBIDDEN, "owner_required"))
    }
}

pub(super) async fn rows(state: &AppState, id: Option<Uuid>) -> Result<Vec<Value>, Error> {
    sqlx::query_scalar("SELECT json_build_object('id',id,'instanceId',instance_id,'generation',generation,'kind',kind,'state',state,'attempt',attempt,'cancelRequested',cancel_requested,'progress',progress,'bytes',artifact_bytes,'sha256',artifact_sha256,'manifest',manifest,'errorCode',error_code,'createdAt',created_at,'startedAt',started_at,'finishedAt',finished_at) FROM instance_operations WHERE ($1::uuid IS NULL OR id=$1) ORDER BY created_at DESC,id DESC LIMIT 100")
        .bind(id).fetch_all(&state.pool).await.map_err(internal)
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
) -> Reply {
    owner(&ctx)?;
    Ok(Json(
        json!({"items":rows(&state,None).await?,"configured":operations::configured(&state)}),
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Create {
    command_id: Uuid,
    instance_id: Uuid,
    generation: i64,
    recovery_key: String,
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Json(body): Json<Create>,
) -> Reply {
    owner(&ctx)?;
    if !operations::configured(&state) {
        return Err(fail(StatusCode::CONFLICT, "backup_not_configured"));
    }
    let key = sentori_snapshot::parse_key(&body.recovery_key)
        .map_err(|_| fail(StatusCode::BAD_REQUEST, "invalid_recovery_key"))?;
    let hash = hex::encode(Sha256::digest(format!(
        "backup:{}:{}:{}:{}",
        ctx.user_id,
        body.instance_id,
        body.generation,
        sentori_snapshot::fingerprint_key(&key)
    )));
    let mut tx = state.pool.begin().await.map_err(internal)?;
    let (id, generation, mode): (Uuid, i64, String) = sqlx::query_as(
        "SELECT id,generation,operating_mode FROM instance_settings WHERE singleton FOR UPDATE",
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(internal)?;
    if id != body.instance_id || generation != body.generation || mode != "normal" {
        return Err(fail(StatusCode::CONFLICT, "instance_changed"));
    }
    let existing: Option<(Uuid, String)> =
        sqlx::query_as("SELECT id,request_hash FROM instance_operations WHERE command_id=$1")
            .bind(body.command_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(internal)?;
    if let Some((id, previous)) = existing {
        if previous != hash {
            return Err(fail(StatusCode::CONFLICT, "idempotency_conflict"));
        }
        tx.commit().await.map_err(internal)?;
        return Ok(Json(
            json!({"operation":rows(&state,Some(id)).await?.first()}),
        ));
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
    let sealed =
        operations::seal_key(&state, operation, &hex::encode(key.as_bytes())).map_err(internal)?;
    sqlx::query("INSERT INTO instance_operations (id,instance_id,generation,actor_user_id,command_id,request_hash,kind,key_blob) VALUES ($1,$2,$3,$4,$5,$6,'backup',$7)")
        .bind(operation).bind(id).bind(generation).bind(ctx.user_id).bind(body.command_id).bind(hash).bind(sealed).execute(&mut *tx).await.map_err(internal)?;
    sqlx::query("INSERT INTO audit_logs (id,actor_user_id,action,target_type,target_id,payload,ip) VALUES ($1,$2,'instance.backup.requested','instance_operation',$3,$4,$5)")
        .bind(Uuid::now_v7()).bind(ctx.user_id).bind(operation.to_string()).bind(json!({"instanceId":id,"generation":generation})).bind(sentori_request_origin::current()).execute(&mut *tx).await.map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok(Json(
        json!({"operation":rows(&state,Some(operation)).await?.first()}),
    ))
}

pub async fn cancel(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(id): Path<Uuid>,
) -> Reply {
    owner(&ctx)?;
    let mut tx = state.pool.begin().await.map_err(internal)?;
    let changed=sqlx::query("UPDATE instance_operations SET cancel_requested=true,state=CASE WHEN state='queued' THEN 'cancelled' ELSE state END,key_blob=CASE WHEN state='queued' THEN NULL ELSE key_blob END,finished_at=CASE WHEN state='queued' THEN now() ELSE finished_at END WHERE id=$1 AND state IN ('queued','running') AND NOT cancel_requested")
        .bind(id).execute(&mut *tx).await.map_err(internal)?.rows_affected();
    if changed == 1 {
        sqlx::query("INSERT INTO audit_logs (id,actor_user_id,action,target_type,target_id,payload,ip) VALUES ($1,$2,'instance.backup.cancel','instance_operation',$3,'{}',$4)")
            .bind(Uuid::now_v7()).bind(ctx.user_id).bind(id.to_string()).bind(sentori_request_origin::current()).execute(&mut *tx).await.map_err(internal)?;
    }
    tx.commit().await.map_err(internal)?;
    let row = rows(&state, Some(id))
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| fail(StatusCode::NOT_FOUND, "operation_not_found"))?;
    Ok(Json(json!({"operation":row})))
}

pub async fn download(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(id): Path<Uuid>,
) -> Result<Response, Error> {
    owner(&ctx)?;
    let artifact:Option<(Uuid,i64,String)>=sqlx::query_as("SELECT artifact_id,artifact_bytes,artifact_sha256 FROM instance_operations WHERE id=$1 AND state='ready'")
        .bind(id).fetch_optional(&state.pool).await.map_err(internal)?;
    let (artifact, bytes, sha) =
        artifact.ok_or_else(|| fail(StatusCode::NOT_FOUND, "backup_unavailable"))?;
    let (_, directory) = operations::directories()
        .ok_or_else(|| fail(StatusCode::CONFLICT, "backup_not_configured"))?;
    let file = tokio::fs::File::open(directory.join(format!("{artifact}.sentori")))
        .await
        .map_err(|_| fail(StatusCode::NOT_FOUND, "backup_unavailable"))?;
    if file.metadata().await.map_err(internal)?.len() != u64::try_from(bytes).map_err(internal)? {
        return Err(fail(StatusCode::CONFLICT, "backup_corrupt"));
    }
    sqlx::query("INSERT INTO audit_logs (id,actor_user_id,action,target_type,target_id,payload,ip) VALUES ($1,$2,'instance.backup.download_started','instance_operation',$3,'{}',$4)")
        .bind(Uuid::now_v7()).bind(ctx.user_id).bind(id.to_string()).bind(sentori_request_origin::current()).execute(&state.pool).await.map_err(internal)?;
    let stream = futures::stream::try_unfold(file, |mut file| async move {
        use tokio::io::AsyncReadExt;
        let mut buffer = vec![0u8; 512 * 1024];
        let count = file.read(&mut buffer).await?;
        buffer.truncate(count);
        Ok::<_, std::io::Error>(if count == 0 {
            None
        } else {
            Some((buffer, file))
        })
    });
    Ok((
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=sentori-{id}.sentori"),
            ),
            (header::CONTENT_LENGTH, bytes.to_string()),
            (header::CACHE_CONTROL, "no-store".to_owned()),
            (header::ETAG, format!("\"{sha}\"")),
        ],
        Body::from_stream(stream),
    )
        .into_response())
}
