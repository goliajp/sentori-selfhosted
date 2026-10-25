//! Bounded encrypted uploads. A file is persisted and independently verified by
//! the durable worker before it is exposed as ready. Plaintext is never extracted.
use super::instance_operations::{Reply, fail, internal, owner, rows};
use crate::{instance_operations as operations, session_mw::SessionContext, state::AppState};
use axum::{
    Extension, Json,
    extract::{Multipart, State},
    http::StatusCode,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;
/// Browser uploads are capped at 1 GiB; the streaming operator verifier supports
/// the archive format's full 64 GiB limit without passing through a reverse proxy.
pub const UPLOAD_LIMIT: usize = 1024 * 1024 * 1024;

pub(super) struct Upload {
    pub(super) command: Uuid,
    pub(super) instance: Uuid,
    pub(super) generation: i64,
    pub(super) key: sentori_snapshot::RecoveryKey,
    pub(super) file: tempfile::NamedTempFile,
    pub(super) bytes: i64,
    pub(super) sha: String,
}

pub(super) async fn read_upload(
    mut multipart: Multipart,
    directory: std::path::PathBuf,
) -> Result<Upload, super::instance_operations::Error> {
    tokio::fs::create_dir_all(&directory)
        .await
        .map_err(internal)?;
    let file = tempfile::NamedTempFile::new_in(&directory).map_err(internal)?;
    let mut output = tokio::fs::File::from_std(file.reopen().map_err(internal)?);
    let mut fields = std::collections::BTreeMap::new();
    let mut size = 0usize;
    let mut hash = Sha256::new();
    let mut seen_file = false;
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|_| fail(StatusCode::BAD_REQUEST, "invalid_backup_upload"))?
    {
        let name = field.name().unwrap_or_default().to_owned();
        if name == "archive" {
            if seen_file {
                return Err(fail(StatusCode::BAD_REQUEST, "invalid_backup_upload"));
            }
            seen_file = true;
            while let Some(chunk) = field
                .chunk()
                .await
                .map_err(|_| fail(StatusCode::BAD_REQUEST, "invalid_backup_upload"))?
            {
                size = size.checked_add(chunk.len()).ok_or_else(|| {
                    fail(StatusCode::PAYLOAD_TOO_LARGE, "backup_upload_too_large")
                })?;
                if size > UPLOAD_LIMIT {
                    return Err(fail(
                        StatusCode::PAYLOAD_TOO_LARGE,
                        "backup_upload_too_large",
                    ));
                }
                hash.update(&chunk);
                output.write_all(&chunk).await.map_err(internal)?;
            }
        } else {
            if !["commandId", "instanceId", "generation", "recoveryKey"].contains(&name.as_str())
                || fields.contains_key(&name)
            {
                return Err(fail(StatusCode::BAD_REQUEST, "invalid_backup_upload"));
            }
            let mut value = Vec::new();
            while let Some(chunk) = field
                .chunk()
                .await
                .map_err(|_| fail(StatusCode::BAD_REQUEST, "invalid_backup_upload"))?
            {
                if value.len() + chunk.len() > 128 {
                    return Err(fail(StatusCode::BAD_REQUEST, "invalid_backup_upload"));
                }
                value.extend_from_slice(&chunk);
            }
            fields.insert(
                name,
                String::from_utf8(value)
                    .map_err(|_| fail(StatusCode::BAD_REQUEST, "invalid_backup_upload"))?,
            );
        }
    }
    if !seen_file || size == 0 || fields.len() != 4 {
        return Err(fail(StatusCode::BAD_REQUEST, "invalid_backup_upload"));
    }
    output.sync_all().await.map_err(internal)?;
    let invalid = || fail(StatusCode::BAD_REQUEST, "invalid_backup_upload");
    Ok(Upload {
        command: fields["commandId"].parse().map_err(|_| invalid())?,
        instance: fields["instanceId"].parse().map_err(|_| invalid())?,
        generation: fields["generation"].parse().map_err(|_| invalid())?,
        key: sentori_snapshot::parse_key(&fields["recoveryKey"])
            .map_err(|_| fail(StatusCode::BAD_REQUEST, "invalid_recovery_key"))?,
        file,
        bytes: i64::try_from(size).map_err(internal)?,
        sha: hex::encode(hash.finalize()),
    })
}

pub async fn verify_upload(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    multipart: Multipart,
) -> Reply {
    owner(&ctx)?;
    if !operations::configured(&state) {
        return Err(fail(StatusCode::CONFLICT, "backup_not_configured"));
    }
    let upload = tokio::time::timeout(
        std::time::Duration::from_mins(30),
        read_upload(
            multipart,
            operations::directories()
                .ok_or_else(|| fail(StatusCode::CONFLICT, "backup_not_configured"))?
                .1,
        ),
    )
    .await
    .map_err(|_| fail(StatusCode::REQUEST_TIMEOUT, "backup_upload_timeout"))??;
    let hash = hex::encode(Sha256::digest(format!(
        "verify:{}:{}:{}:{}:{}",
        ctx.user_id,
        upload.instance,
        upload.generation,
        upload.sha,
        sentori_snapshot::fingerprint_key(&upload.key)
    )));
    let mut tx = state.pool.begin().await.map_err(internal)?;
    let (id, generation, mode): (Uuid, i64, String) = sqlx::query_as(
        "SELECT id,generation,operating_mode FROM instance_settings WHERE singleton FOR UPDATE",
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(internal)?;
    if id != upload.instance || generation != upload.generation || mode != "normal" {
        return Err(fail(StatusCode::CONFLICT, "instance_changed"));
    }
    let existing: Option<(Uuid, String)> =
        sqlx::query_as("SELECT id,request_hash FROM instance_operations WHERE command_id=$1")
            .bind(upload.command)
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
    let sealed = operations::seal_key(&state, operation, &hex::encode(upload.key.as_bytes()))
        .map_err(internal)?;
    sqlx::query("INSERT INTO instance_operations (id,instance_id,generation,actor_user_id,command_id,request_hash,kind,key_blob,artifact_id,artifact_bytes,artifact_sha256) VALUES ($1,$2,$3,$4,$5,$6,'verify',$7,$1,$8,$9)")
        .bind(operation).bind(id).bind(generation).bind(ctx.user_id).bind(upload.command).bind(hash).bind(sealed).bind(upload.bytes).bind(&upload.sha).execute(&mut *tx).await.map_err(internal)?;
    sqlx::query("INSERT INTO audit_logs (id,actor_user_id,action,target_type,target_id,payload,ip) VALUES ($1,$2,'instance.backup.uploaded','instance_operation',$3,$4,$5)")
        .bind(Uuid::now_v7()).bind(ctx.user_id).bind(operation.to_string()).bind(json!({"bytes":upload.bytes,"sha256":upload.sha})).bind(sentori_request_origin::current()).execute(&mut *tx).await.map_err(internal)?;
    let (_, directory) = operations::directories()
        .ok_or_else(|| fail(StatusCode::CONFLICT, "backup_not_configured"))?;
    upload
        .file
        .persist_noclobber(directory.join(format!("{operation}.sentori")))
        .map_err(internal)?;
    std::fs::File::open(directory)
        .and_then(|file| file.sync_all())
        .map_err(internal)?;
    // Preserve the encrypted file if COMMIT's outcome is ambiguous.
    tx.commit().await.map_err(internal)?;
    Ok(Json(
        json!({"operation":rows(&state,Some(operation)).await?.first()}),
    ))
}
