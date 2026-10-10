use super::{
    instance_operations::{Reply, fail, internal, owner},
    instance_upload,
    project_transfers::{active_job, identity, receipt, replay, request_hash},
};
use crate::{
    instance_operations, project_transfers as transfers, session_mw::SessionContext,
    state::AppState,
};
use axum::{
    Extension, Json,
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
};
use serde::Deserialize;
use serde_json::json;
use std::{sync::Arc, time::Duration};
use uuid::Uuid;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Phase {
    phase: String,
    version: i64,
}
pub async fn upload(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(id): Path<Uuid>,
    Query(phase): Query<Phase>,
    multipart: Multipart,
) -> Reply {
    owner(&ctx)?;
    if !transfers::configured(&state) {
        return Err(fail(StatusCode::CONFLICT, "transfer_not_configured"));
    }
    if !matches!(phase.phase.as_str(), "copy" | "final") {
        return Err(fail(StatusCode::BAD_REQUEST, "invalid_transfer_phase"));
    }
    let directory = transfers::prepare_directory().map_err(internal)?;
    let upload = tokio::time::timeout(
        Duration::from_mins(30),
        instance_upload::read_upload(multipart, directory.clone()),
    )
    .await
    .map_err(|_| fail(StatusCode::REQUEST_TIMEOUT, "transfer_upload_timeout"))??;
    let hash = request_hash(
        ctx.user_id,
        "import",
        &json!({"transferId":id,"instanceId":upload.instance,"generation":upload.generation,"sha256":upload.sha,"key":sentori_snapshot::fingerprint_key(&upload.key),"phase":phase.phase,"version":phase.version}),
    )?;
    let mut tx = state.pool.begin().await.map_err(internal)?;
    identity(&mut tx, upload.instance, upload.generation).await?;
    if let Some(response) = replay(&mut tx, upload.command, &hash).await? {
        return Ok(Json(response));
    }
    let transfer = transfers::locked(&mut tx, id)
        .await
        .map_err(|_| fail(StatusCode::NOT_FOUND, "transfer_not_found"))?;
    let version: i64 = sqlx::query_scalar("SELECT version FROM project_transfers WHERE id=$1")
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .map_err(internal)?;
    if version != phase.version
        || (transfer.instance_id, transfer.generation) != (upload.instance, upload.generation)
    {
        return Err(fail(StatusCode::CONFLICT, "transfer_changed"));
    }
    if transfer.direction != "target"
        || !matches!(transfer.state.as_str(), "copying" | "copied")
        || (phase.phase == "final" && transfer.state != "copied")
    {
        return Err(fail(StatusCode::CONFLICT, "invalid_transfer_transition"));
    }
    active_job(&mut tx, id).await?;
    let job = Uuid::now_v7();
    let sealed = instance_operations::seal_key(&state, job, &hex::encode(upload.key.as_bytes()))
        .map_err(internal)?;
    sqlx::query("INSERT INTO project_transfer_jobs(id,transfer_id,actor_user_id,command_id,request_hash,kind,phase,key_blob,artifact_id,artifact_bytes,artifact_sha256) VALUES($1,$2,$3,$4,$5,'import',$6,$7,$1,$8,$9)")
        .bind(job).bind(id).bind(ctx.user_id).bind(upload.command).bind(&hash).bind(&phase.phase).bind(sealed).bind(upload.bytes).bind(&upload.sha).execute(&mut *tx).await.map_err(internal)?;
    sqlx::query("UPDATE project_transfers SET version=version+1,updated_at=now() WHERE id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    let response = json!({"transferId":id,"jobId":job,"version":version+1});
    receipt(&mut tx, upload.command, id, ctx.user_id, &hash, &response).await?;
    transfers::audit(
        &mut tx,
        Some(ctx.user_id),
        id,
        "transfer.uploaded",
        json!({"jobId":job,"phase":phase.phase,"bytes":upload.bytes,"sha256":upload.sha}),
    )
    .await
    .map_err(internal)?;
    upload
        .file
        .persist_noclobber(directory.join(format!("{job}.sentori")))
        .map_err(internal)?;
    std::fs::File::open(directory)
        .and_then(|file| file.sync_all())
        .map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok(Json(response))
}
