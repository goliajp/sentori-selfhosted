//! Each side authorizes its own owner. Only a bound encrypted source archive can
//! fill the independently authorized target receipt; credentials never migrate.
use super::instance_operations::{Error, Reply, fail, internal, owner};
use crate::{
    instance_operations, project_transfers as transfers, session_mw::SessionContext,
    state::AppState,
};
use axum::{
    Extension, Json,
    body::Body,
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Transaction};
use std::{collections::BTreeMap, sync::Arc};
use uuid::Uuid;

pub(super) async fn rows(state: &AppState, id: Option<Uuid>) -> Result<Vec<Value>, Error> {
    sqlx::query_scalar("SELECT jsonb_build_object('id',t.id,'direction',direction,'instanceId',instance_id,'generation',generation,'sourceInstanceId',source_instance_id,'sourceGeneration',source_generation,'targetInstanceId',target_instance_id,'targetGeneration',target_generation,'projectIds',project_ids,'projectMapping',project_mapping,'state',state,'version',version,'manifest',latest_manifest,'createdAt',created_at,'updatedAt',updated_at,'sealedAt',sealed_at,'activatedAt',activated_at,'observedEvents',observed_events,'verifiedEvents',verified_events,'rollbackPlan',rollback_plan,'jobs',coalesce((SELECT jsonb_agg(jsonb_build_object('id',j.id,'kind',j.kind,'phase',j.phase,'state',j.state,'cancelRequested',j.cancel_requested,'attempt',j.attempt,'progress',j.progress,'bytes',j.artifact_bytes,'sha256',j.artifact_sha256,'errorCode',j.error_code,'createdAt',j.created_at) ORDER BY j.created_at DESC) FROM project_transfer_jobs j WHERE j.transfer_id=t.id),'[]'::jsonb),'routes',coalesce((SELECT jsonb_agg(jsonb_build_object('projectId',r.project_id,'targetProjectId',r.target_project_id,'origin',r.target_origin,'enabled',r.enabled,'expiresAt',r.expires_at,'forwarded',r.forwarded,'failed',r.failed,'lastSeenAt',r.last_seen_at,'lastError',r.last_error)) FROM project_transfer_routes r WHERE r.transfer_id=t.id),'[]'::jsonb)) FROM project_transfers t WHERE ($1::uuid IS NULL OR t.id=$1) ORDER BY t.created_at DESC,t.id DESC LIMIT 100")
        .bind(id).fetch_all(&state.pool).await.map_err(internal)
}
pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
) -> Reply {
    owner(&ctx)?;
    Ok(Json(
        json!({"items":rows(&state,None).await?,"configured":transfers::configured(&state),"uploadLimit":super::instance_upload::UPLOAD_LIMIT}),
    ))
}
pub(super) async fn identity(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    generation: i64,
) -> Result<(), Error> {
    let actual: (Uuid, i64, String) = sqlx::query_as(
        "SELECT id,generation,operating_mode FROM instance_settings WHERE singleton FOR UPDATE",
    )
    .fetch_one(&mut **tx)
    .await
    .map_err(internal)?;
    if actual != (id, generation, "normal".into()) {
        return Err(fail(StatusCode::CONFLICT, "instance_changed"));
    }
    Ok(())
}
pub(super) async fn replay(
    tx: &mut Transaction<'_, Postgres>,
    command: Uuid,
    hash: &str,
) -> Result<Option<Value>, Error> {
    let existing: Option<(String, Value)> = sqlx::query_as(
        "SELECT request_hash,response FROM project_transfer_commands WHERE command_id=$1",
    )
    .bind(command)
    .fetch_optional(&mut **tx)
    .await
    .map_err(internal)?;
    match existing {
        Some((old, response)) if old == hash => Ok(Some(response)),
        Some(_) => Err(fail(StatusCode::CONFLICT, "idempotency_conflict")),
        None => Ok(None),
    }
}
pub(super) async fn receipt(
    tx: &mut Transaction<'_, Postgres>,
    command: Uuid,
    transfer: Uuid,
    actor: Uuid,
    hash: &str,
    response: &Value,
) -> Result<(), Error> {
    sqlx::query("INSERT INTO project_transfer_commands(command_id,transfer_id,actor_user_id,request_hash,response) VALUES($1,$2,$3,$4,$5)")
        .bind(command).bind(transfer).bind(actor).bind(hash).bind(response).execute(&mut **tx).await.map_err(internal)?;
    Ok(())
}
pub(super) fn request_hash(
    actor: Uuid,
    action: &str,
    body: &impl Serialize,
) -> Result<String, Error> {
    let bytes = serde_json::to_vec(&json!({"actor":actor,"action":action,"body":body}))
        .map_err(internal)?;
    Ok(hex::encode(Sha256::digest(bytes)))
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Create {
    command_id: Uuid,
    id: Uuid,
    direction: String,
    instance_id: Uuid,
    generation: i64,
    source_instance_id: Uuid,
    source_generation: i64,
    target_instance_id: Uuid,
    target_generation: i64,
    project_ids: Vec<Uuid>,
}
pub async fn create(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Json(mut body): Json<Create>,
) -> Reply {
    owner(&ctx)?;
    if !transfers::configured(&state) {
        return Err(fail(StatusCode::CONFLICT, "transfer_not_configured"));
    }
    if body.direction == "source" && state.cloud.is_some() {
        return Err(fail(StatusCode::CONFLICT, "self_hosted_source_required"));
    }
    body.project_ids.sort_unstable();
    if !matches!(body.direction.as_str(), "source" | "target")
        || body.id.is_nil()
        || body.source_instance_id.is_nil()
        || body.target_instance_id.is_nil()
        || body.source_instance_id == body.target_instance_id
        || body.source_generation < 0
        || body.target_generation < 0
        || body.project_ids.is_empty()
        || body.project_ids.len() > 1000
        || body.project_ids.iter().any(Uuid::is_nil)
        || body.project_ids.windows(2).any(|ids| ids[0] == ids[1])
    {
        return Err(fail(StatusCode::BAD_REQUEST, "invalid_transfer"));
    }
    let expected = if body.direction == "source" {
        (body.source_instance_id, body.source_generation)
    } else {
        (body.target_instance_id, body.target_generation)
    };
    if expected != (body.instance_id, body.generation) {
        return Err(fail(StatusCode::CONFLICT, "instance_changed"));
    }
    let hash = request_hash(ctx.user_id, "create", &body)?;
    let mut tx = state.pool.begin().await.map_err(internal)?;
    identity(&mut tx, body.instance_id, body.generation).await?;
    if let Some(response) = replay(&mut tx, body.command_id, &hash).await? {
        return Ok(Json(response));
    }
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM project_transfers WHERE id=$1)")
            .bind(body.id)
            .fetch_one(&mut *tx)
            .await
            .map_err(internal)?;
    if exists {
        return Err(fail(StatusCode::CONFLICT, "transfer_exists"));
    }
    if body.direction == "source" {
        let projects:Vec<Uuid>=sqlx::query_scalar("SELECT id FROM projects WHERE id=ANY($1) AND migration_fence IS NULL ORDER BY id FOR UPDATE").bind(&body.project_ids).fetch_all(&mut *tx).await.map_err(internal)?;
        let overlap:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM project_transfers WHERE direction='source' AND state<>'cancelled' AND project_ids && $1)").bind(&body.project_ids).fetch_one(&mut *tx).await.map_err(internal)?;
        if projects != body.project_ids || overlap {
            return Err(fail(StatusCode::CONFLICT, "projects_unavailable"));
        }
    }
    sqlx::query("INSERT INTO project_transfers(id,direction,instance_id,generation,source_instance_id,source_generation,target_instance_id,target_generation,actor_user_id,project_ids) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)")
        .bind(body.id).bind(&body.direction).bind(body.instance_id).bind(body.generation).bind(body.source_instance_id).bind(body.source_generation).bind(body.target_instance_id).bind(body.target_generation).bind(ctx.user_id).bind(&body.project_ids).execute(&mut *tx).await.map_err(internal)?;
    let response = json!({"transferId":body.id,"version":1});
    receipt(
        &mut tx,
        body.command_id,
        body.id,
        ctx.user_id,
        &hash,
        &response,
    )
    .await?;
    transfers::audit(
        &mut tx,
        Some(ctx.user_id),
        body.id,
        "transfer.created",
        json!({"direction":body.direction,"projectIds":body.project_ids}),
    )
    .await
    .map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok(Json(response))
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommandRequest {
    command_id: Uuid,
    instance_id: Uuid,
    generation: i64,
    version: i64,
    action: String,
    #[serde(default)]
    recovery_key: Option<String>,
    #[serde(default)]
    rollback_plan: Option<String>,
    #[serde(default)]
    sdk_updated: bool,
    #[serde(default)]
    events: BTreeMap<Uuid, Uuid>,
}
pub(super) async fn active_job(tx: &mut Transaction<'_, Postgres>, id: Uuid) -> Result<(), Error> {
    let busy:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM project_transfer_jobs WHERE transfer_id=$1 AND state IN ('queued','running'))").bind(id).fetch_one(&mut **tx).await.map_err(internal)?;
    if busy {
        return Err(fail(StatusCode::CONFLICT, "transfer_job_in_progress"));
    }
    Ok(())
}
// Keep the authorized state transitions and their single atomic receipt together.
#[allow(clippy::too_many_lines)]
pub async fn command(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(id): Path<Uuid>,
    Json(body): Json<CommandRequest>,
) -> Reply {
    owner(&ctx)?;
    let hash = request_hash(ctx.user_id, &format!("command:{id}"), &body)?;
    let mut tx = state.pool.begin().await.map_err(internal)?;
    identity(&mut tx, body.instance_id, body.generation).await?;
    if let Some(response) = replay(&mut tx, body.command_id, &hash).await? {
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
    if version != body.version
        || (transfer.instance_id, transfer.generation) != (body.instance_id, body.generation)
    {
        return Err(fail(StatusCode::CONFLICT, "transfer_changed"));
    }
    active_job(&mut tx, id).await?;
    let bad = || fail(StatusCode::CONFLICT, "invalid_transfer_transition");
    let mut next = transfer.state.clone();
    let mut job_id = None;
    sqlx::query("SELECT set_config('sentori.transfer_id',$1,true)")
        .bind(id.to_string())
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    match body.action.as_str() {
        "export_copy" | "export_final" => {
            let final_copy = body.action == "export_final";
            if transfer.direction != "source"
                || if final_copy {
                    transfer.state != "sealed"
                } else {
                    !matches!(transfer.state.as_str(), "copying" | "copied")
                }
            {
                return Err(bad());
            }
            let key = sentori_snapshot::parse_key(body.recovery_key.as_deref().unwrap_or_default())
                .map_err(|_| fail(StatusCode::BAD_REQUEST, "invalid_recovery_key"))?;
            let job = Uuid::now_v7();
            let sealed = instance_operations::seal_key(&state, job, &hex::encode(key.as_bytes()))
                .map_err(internal)?;
            sqlx::query("INSERT INTO project_transfer_jobs(id,transfer_id,actor_user_id,command_id,request_hash,kind,phase,key_blob) VALUES($1,$2,$3,$4,$5,'export',$6,$7)")
                .bind(job).bind(id).bind(ctx.user_id).bind(body.command_id).bind(&hash).bind(if final_copy {"final"}else{"copy"}).bind(sealed).execute(&mut *tx).await.map_err(internal)?;
            job_id = Some(job);
        }
        "seal" => {
            let plan = body.rollback_plan.as_deref().unwrap_or_default().trim();
            if transfer.direction != "source" || transfer.state != "copied" {
                return Err(bad());
            }
            if !(30..=8000).contains(&plan.len()) {
                return Err(fail(StatusCode::BAD_REQUEST, "rollback_plan_required"));
            }
            let changed=sqlx::query("UPDATE projects SET migration_fence=$2 WHERE id=ANY($1) AND migration_fence IS NULL").bind(&transfer.project_ids).bind(id).execute(&mut *tx).await.map_err(internal)?.rows_affected();
            if usize::try_from(changed).map_err(internal)? != transfer.project_ids.len() {
                return Err(fail(StatusCode::CONFLICT, "projects_unavailable"));
            }
            sqlx::query(
                "UPDATE project_transfers SET sealed_at=now(),rollback_plan=$2 WHERE id=$1",
            )
            .bind(id)
            .bind(plan)
            .execute(&mut *tx)
            .await
            .map_err(internal)?;
            next = "sealed".into();
        }
        "activate" => {
            let plan = body.rollback_plan.as_deref().unwrap_or_default().trim();
            if transfer.direction != "target" || transfer.state != "ready" {
                return Err(bad());
            }
            if !(30..=8000).contains(&plan.len()) {
                return Err(fail(StatusCode::BAD_REQUEST, "rollback_plan_required"));
            }
            let changed =
                sqlx::query("UPDATE projects SET migration_fence=NULL WHERE migration_fence=$1")
                    .bind(id)
                    .execute(&mut *tx)
                    .await
                    .map_err(internal)?
                    .rows_affected();
            if usize::try_from(changed).map_err(internal)? != transfer.project_ids.len() {
                return Err(fail(StatusCode::CONFLICT, "projects_unavailable"));
            }
            sqlx::query("UPDATE project_transfers SET activated_at=clock_timestamp(),rollback_plan=$2 WHERE id=$1").bind(id).bind(plan).execute(&mut *tx).await.map_err(internal)?;
            next = "active".into();
        }
        "verify" => {
            if transfer.direction != "target"
                || !matches!(transfer.state.as_str(), "active" | "observing")
                || body.events.len() != transfer.project_ids.len()
            {
                return Err(bad());
            }
            for source in &transfer.project_ids {
                let event = body
                    .events
                    .get(source)
                    .ok_or_else(|| fail(StatusCode::BAD_REQUEST, "event_evidence_required"))?;
                let target = sentori_snapshot::project_import::mapped_id(id, "projects", *source);
                let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM events e JOIN project_transfers t ON t.id=$3 WHERE e.project_id=$1 AND e.id=$2 AND e.received_at>t.activated_at AND t.observed_events->>$4=$2::text)")
                    .bind(target).bind(event).bind(id).bind(source.to_string()).fetch_one(&mut *tx).await.map_err(internal)?;
                if !exists {
                    return Err(fail(StatusCode::CONFLICT, "target_event_not_received"));
                }
            }
            sqlx::query("UPDATE project_transfers SET verified_events=$2 WHERE id=$1")
                .bind(id)
                .bind(serde_json::to_value(&body.events).map_err(internal)?)
                .execute(&mut *tx)
                .await
                .map_err(internal)?;
            next = "observing".into();
        }
        "complete" => {
            if transfer.direction != "target" || transfer.state != "observing" || !body.sdk_updated
            {
                return Err(bad());
            }
            next = "completed".into();
            sqlx::query("UPDATE project_transfers SET completed_at=now() WHERE id=$1")
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(internal)?;
        }
        "cancel" => {
            if !matches!(transfer.state.as_str(), "copying" | "copied") {
                return Err(bad());
            }
            if transfer.direction == "target" {
                sqlx::query("DELETE FROM projects WHERE migration_fence=$1")
                    .bind(id)
                    .execute(&mut *tx)
                    .await
                    .map_err(internal)?;
            }
            next = "cancelled".into();
        }
        _ => return Err(fail(StatusCode::BAD_REQUEST, "invalid_transfer_action")),
    }
    sqlx::query(
        "UPDATE project_transfers SET state=$2,version=version+1,updated_at=now() WHERE id=$1",
    )
    .bind(id)
    .bind(&next)
    .execute(&mut *tx)
    .await
    .map_err(internal)?;
    let response = json!({"transferId":id,"version":version+1,"state":next,"jobId":job_id});
    receipt(&mut tx, body.command_id, id, ctx.user_id, &hash, &response).await?;
    transfers::audit(
        &mut tx,
        Some(ctx.user_id),
        id,
        &format!("transfer.{}", body.action),
        json!({"version":version+1,"jobId":job_id}),
    )
    .await
    .map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok(Json(response))
}
pub async fn download(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path((id, job)): Path<(Uuid, Uuid)>,
) -> Result<Response, Error> {
    owner(&ctx)?;
    let item:Option<(Uuid,i64,String)>=sqlx::query_as("SELECT artifact_id,artifact_bytes,artifact_sha256 FROM project_transfer_jobs WHERE id=$1 AND transfer_id=$2 AND kind='export' AND state='ready'").bind(job).bind(id).fetch_optional(&state.pool).await.map_err(internal)?;
    let (artifact, bytes, sha) =
        item.ok_or_else(|| fail(StatusCode::NOT_FOUND, "transfer_archive_unavailable"))?;
    let (_, directory) = transfers::directories()
        .ok_or_else(|| fail(StatusCode::CONFLICT, "transfer_not_configured"))?;
    let file = tokio::fs::File::open(directory.join(format!("{artifact}.sentori")))
        .await
        .map_err(|_| fail(StatusCode::NOT_FOUND, "transfer_archive_unavailable"))?;
    if file.metadata().await.map_err(internal)?.len() != u64::try_from(bytes).map_err(internal)? {
        return Err(fail(StatusCode::CONFLICT, "transfer_archive_corrupt"));
    }
    let stream = futures::stream::try_unfold(file, |mut file| async move {
        use tokio::io::AsyncReadExt;
        let mut buffer = vec![0; 512 * 1024];
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
                format!("attachment; filename=sentori-transfer-{id}-{job}.sentori"),
            ),
            (header::CONTENT_LENGTH, bytes.to_string()),
            (header::CACHE_CONTROL, "no-store".into()),
            (header::ETAG, format!("\"{sha}\"")),
        ],
        Body::from_stream(stream),
    )
        .into_response())
}

pub async fn cancel_job(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path((id, job)): Path<(Uuid, Uuid)>,
) -> Reply {
    owner(&ctx)?;
    let mut tx = state.pool.begin().await.map_err(internal)?;
    let changed = sqlx::query("UPDATE project_transfer_jobs SET cancel_requested=true,state=CASE WHEN state='queued' THEN 'cancelled' ELSE state END,key_blob=CASE WHEN state='queued' THEN NULL ELSE key_blob END,finished_at=CASE WHEN state='queued' THEN now() ELSE finished_at END WHERE id=$1 AND transfer_id=$2 AND state IN ('queued','running') AND NOT cancel_requested")
        .bind(job).bind(id).execute(&mut *tx).await.map_err(internal)?.rows_affected();
    if changed == 1 {
        transfers::audit(
            &mut tx,
            Some(ctx.user_id),
            id,
            "transfer.job.cancel",
            json!({"jobId":job}),
        )
        .await
        .map_err(internal)?;
    }
    tx.commit().await.map_err(internal)?;
    Ok(Json(json!({"cancelRequested":changed==1})))
}
