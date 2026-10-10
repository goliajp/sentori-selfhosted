use super::{
    instance_operations::{Reply, fail, internal, owner},
    project_transfers::{identity, receipt, replay, request_hash},
};
use crate::{
    project_transfers as transfers, session_mw::SessionContext, state::AppState, transfer_tail,
};
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use sentori_ingest_token::{IngestContext, Scope};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Configure {
    command_id: Uuid,
    instance_id: Uuid,
    generation: i64,
    version: i64,
    project_id: Uuid,
    target_origin: String,
    target_token: String,
    expires_in_days: i32,
}
pub async fn configure(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(id): Path<Uuid>,
    Json(body): Json<Configure>,
) -> Reply {
    owner(&ctx)?;
    if !(1..=90).contains(&body.expires_in_days) {
        return Err(fail(StatusCode::BAD_REQUEST, "invalid_tail_expiry"));
    }
    let origin = transfer_tail::origin(&body.target_origin)
        .map_err(|code| fail(StatusCode::BAD_REQUEST, code))?;
    let hash = request_hash(ctx.user_id, &format!("tail:{id}"), &body)?;
    // Verify the immutable retry receipt before making a network request.
    {
        let mut tx = state.pool.begin().await.map_err(internal)?;
        identity(&mut tx, body.instance_id, body.generation).await?;
        if let Some(value) = replay(&mut tx, body.command_id, &hash).await? {
            return Ok(Json(value));
        }
    }
    let proof = transfer_tail::check_target(&origin, &body.target_token, id)
        .await
        .map_err(|code| fail(StatusCode::CONFLICT, code))?;
    let mut tx = state.pool.begin().await.map_err(internal)?;
    identity(&mut tx, body.instance_id, body.generation).await?;
    if let Some(value) = replay(&mut tx, body.command_id, &hash).await? {
        return Ok(Json(value));
    }
    let transfer = transfers::locked(&mut tx, id)
        .await
        .map_err(|_| fail(StatusCode::NOT_FOUND, "transfer_not_found"))?;
    let (version, archive): (i64, Option<Uuid>) =
        sqlx::query_as("SELECT version,latest_archive_id FROM project_transfers WHERE id=$1")
            .bind(id)
            .fetch_one(&mut *tx)
            .await
            .map_err(internal)?;
    if version != body.version
        || (transfer.instance_id, transfer.generation) != (body.instance_id, body.generation)
    {
        return Err(fail(StatusCode::CONFLICT, "transfer_changed"));
    }
    let target = sentori_snapshot::project_import::mapped_id(id, "projects", body.project_id);
    if transfer.direction != "source"
        || !matches!(transfer.state.as_str(), "sealed" | "observing")
        || !transfer.project_ids.contains(&body.project_id)
        || archive.is_none()
        || proof["transferId"] != id.to_string()
        || proof["instanceId"] != transfer.target_instance_id.to_string()
        || proof["generation"] != transfer.target_generation
        || proof["projectId"] != target.to_string()
        || proof["archiveId"] != json!(archive)
    {
        return Err(fail(StatusCode::CONFLICT, "target_proof_mismatch"));
    }
    let vault = state
        .vault
        .as_ref()
        .ok_or_else(|| fail(StatusCode::CONFLICT, "transfer_not_configured"))?;
    let secret=vault.seal(&serde_json::to_vec(&json!({"transferId":id,"projectId":body.project_id,"targetProjectId":target,"origin":origin,"token":body.target_token})).map_err(internal)?).map_err(internal)?;
    sqlx::query("INSERT INTO project_transfer_routes(project_id,transfer_id,target_project_id,target_origin,sealed_token,expires_at) VALUES($1,$2,$3,$4,$5,clock_timestamp()+make_interval(days=>$6)) ON CONFLICT(project_id) DO UPDATE SET target_project_id=excluded.target_project_id,target_origin=excluded.target_origin,sealed_token=excluded.sealed_token,expires_at=excluded.expires_at,enabled=true WHERE project_transfer_routes.transfer_id=excluded.transfer_id")
        .bind(body.project_id).bind(id).bind(target).bind(&origin).bind(secret).bind(body.expires_in_days).execute(&mut *tx).await.map_err(internal)?;
    sqlx::query("UPDATE project_transfers SET state='observing',version=version+1,updated_at=now() WHERE id=$1").bind(id).execute(&mut *tx).await.map_err(internal)?;
    let response = json!({"transferId":id,"version":version+1,"state":"observing"});
    receipt(&mut tx, body.command_id, id, ctx.user_id, &hash, &response).await?;
    transfers::audit(&mut tx,Some(ctx.user_id),id,"transfer.tail.configured",json!({"projectId":body.project_id,"targetProjectId":target,"origin":origin,"days":body.expires_in_days})).await.map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok(Json(response))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Proof {
    transfer_id: Uuid,
}
pub async fn proof(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<IngestContext>,
    Json(body): Json<Proof>,
) -> Response {
    if ctx.scope != Scope::Ingest {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error":"ingest_token_required"})),
        )
            .into_response();
    }
    let result:Result<Option<Value>,_>=sqlx::query_scalar("SELECT jsonb_build_object('transferId',t.id,'instanceId',t.instance_id,'generation',t.generation,'projectId',$2::uuid,'archiveId',t.latest_archive_id,'state',t.state,'eventVerified',t.verified_events<>'{}'::jsonb) FROM project_transfers t JOIN instance_settings i ON i.singleton AND i.id=t.instance_id AND i.generation=t.generation AND i.operating_mode='normal' JOIN tokens k ON k.id=$3 AND k.created_at>=t.created_at WHERE t.id=$1 AND t.direction='target' AND t.state IN ('active','observing','completed') AND EXISTS(SELECT 1 FROM jsonb_each_text(t.project_mapping) m WHERE m.value=$2::text)")
        .bind(body.transfer_id).bind(ctx.project_id).bind(ctx.token_id).fetch_optional(&state.pool).await;
    match result {
        Ok(Some(value)) => Json(value).into_response(),
        Ok(None) => (
            StatusCode::CONFLICT,
            Json(json!({"error":"target_not_active"})),
        )
            .into_response(),
        Err(_) => transfer_tail::unavailable(),
    }
}
