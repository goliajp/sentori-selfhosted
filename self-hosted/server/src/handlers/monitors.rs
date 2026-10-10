//! Owner-configured monitors; members can inspect only their assigned projects.
use super::jobs::database_error;
use crate::{session_mw::SessionContext, state::AppState};
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;
type Error = (StatusCode, Json<Value>);
fn error(status: StatusCode, code: &str) -> Error {
    (status, Json(json!({"error":code})))
}
const FIELDS: &str = "jsonb_build_object('id',m.id,'projectId',m.project_id,'name',m.name,'url',m.url,'intervalSeconds',m.interval_seconds,'timeoutSeconds',m.timeout_seconds,'expectedStatus',m.expected_status,'failureThreshold',m.failure_threshold,'enabled',m.enabled,'version',m.version,'status',m.status,'incidentOpen',m.incident_id IS NOT NULL,'nextCheckAt',m.next_check_at)";

pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project): Path<Uuid>,
) -> Result<Json<Value>, Error> {
    super::admin::tokens::ensure_project_access(&state, &ctx, project).await?;
    let query = format!(
        "SELECT {FIELDS} || jsonb_build_object('samples',COALESCE(s.samples,'[]'::jsonb),'checks30d',c.total,'availability',c.availability) FROM monitors m LEFT JOIN LATERAL (SELECT jsonb_agg(item ORDER BY checked_at,id) AS samples FROM (SELECT id,checked_at,jsonb_build_object('at',checked_at,'ok',ok,'code',status_code,'latency',latency_ms,'error',error_code) AS item FROM monitor_samples WHERE monitor_id=m.id AND config_version=m.version ORDER BY checked_at DESC,id DESC LIMIT 60) recent) s ON true LEFT JOIN LATERAL (SELECT count(*) AS total,100.0*count(*) FILTER(WHERE ok)/NULLIF(count(*),0) AS availability FROM monitor_samples WHERE monitor_id=m.id AND config_version=m.version AND checked_at>now()-interval '30 days') c ON true WHERE m.project_id=$1 AND m.deleted_at IS NULL ORDER BY m.created_at,m.id LIMIT 100"
    );
    let rows: Vec<Value> = sqlx::query_scalar(sqlx::AssertSqlSafe(query))
        .bind(project)
        .fetch_all(&state.pool)
        .await
        .map_err(database_error)?;
    Ok(Json(json!({"monitors":rows})))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Page {
    offset: Option<i64>,
}
pub async fn history(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path((project, id)): Path<(Uuid, Uuid)>,
    Query(page): Query<Page>,
) -> Result<Json<Value>, Error> {
    super::admin::tokens::ensure_project_access(&state, &ctx, project).await?;
    sqlx::query("SELECT id FROM monitors WHERE id=$1 AND project_id=$2")
        .bind(id)
        .bind(project)
        .fetch_optional(&state.pool)
        .await
        .map_err(database_error)?
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "not_found"))?;
    let offset = page.offset.unwrap_or(0).clamp(0, 1_000_000);
    let rows:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('id',id,'incidentId',incident_id,'kind',kind,'at',created_at) FROM monitor_events WHERE monitor_id=$1 ORDER BY created_at DESC,id DESC LIMIT 51 OFFSET $2").bind(id).bind(offset).fetch_all(&state.pool).await.map_err(database_error)?;
    Ok(Json(
        json!({"events":rows.iter().take(50).collect::<Vec<_>>(),"nextOffset":(rows.len()>50).then_some(offset+50)}),
    ))
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Configuration {
    name: String,
    url: String,
    interval_seconds: i32,
    timeout_seconds: i32,
    expected_status: i32,
    failure_threshold: i32,
    enabled: bool,
}
impl Configuration {
    fn validate(&self) -> Result<(), Error> {
        if self.name.trim().is_empty()
            || self.name.chars().count() > 100
            || !(60..=86400).contains(&self.interval_seconds)
            || !(1..=30).contains(&self.timeout_seconds)
            || !(200..=599).contains(&self.expected_status)
            || (300..400).contains(&self.expected_status)
            || !(1..=10).contains(&self.failure_threshold)
        {
            return Err(error(StatusCode::BAD_REQUEST, "invalid_monitor"));
        }
        crate::public_http::parse(&self.url)
            .map_err(|code| error(StatusCode::BAD_REQUEST, code))?;
        Ok(())
    }
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MonitorCommand {
    action: Action,
    command_id: Uuid,
    id: Uuid,
    expected_version: i64,
    /// null deletes a configuration; creates and edits carry all fields.
    configuration: Option<Configuration>,
}
#[derive(Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    Save,
    Delete,
    Check,
}
pub async fn command(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project): Path<Uuid>,
    Json(body): Json<MonitorCommand>,
) -> Result<Json<Value>, Error> {
    if !ctx.role.is_superadmin() {
        return Err(error(StatusCode::FORBIDDEN, "owner_required"));
    }
    if (body.action == Action::Save) != body.configuration.is_some() {
        return Err(error(StatusCode::BAD_REQUEST, "invalid_command"));
    }
    if let Some(config) = &body.configuration {
        config.validate()?;
    }
    let digest = hex::encode(Sha256::digest(
        serde_json::to_vec(&body).map_err(|_| error(StatusCode::BAD_REQUEST, "invalid_command"))?,
    ));
    let mut tx = state.pool.begin().await.map_err(database_error)?;
    sqlx::query("SELECT id FROM projects WHERE id=$1 FOR NO KEY UPDATE")
        .bind(project)
        .fetch_optional(&mut *tx)
        .await
        .map_err(database_error)?
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "not_found"))?;
    if let Some(row)=sqlx::query("SELECT digest,response FROM monitor_commands WHERE project_id=$1 AND actor_user_id=$2 AND command_id=$3").bind(project).bind(ctx.user_id).bind(body.command_id).fetch_optional(&mut *tx).await.map_err(database_error)?{
        if row.get::<String,_>("digest")!=digest{return Err(error(StatusCode::CONFLICT,"idempotency_conflict"));}
        return Ok(Json(row.get("response")));
    }
    let version:Option<i64>=sqlx::query_scalar("SELECT version FROM monitors WHERE id=$1 AND project_id=$2 AND deleted_at IS NULL FOR UPDATE").bind(body.id).bind(project).fetch_optional(&mut *tx).await.map_err(database_error)?;
    if version.unwrap_or(0) != body.expected_version
        || (version.is_none() && body.configuration.is_none())
    {
        return Err(error(StatusCode::CONFLICT, "monitor_changed"));
    }
    if version.is_none() {
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM monitors WHERE project_id=$1 AND deleted_at IS NULL",
        )
        .bind(project)
        .fetch_one(&mut *tx)
        .await
        .map_err(database_error)?;
        if count >= 100 {
            return Err(error(StatusCode::CONFLICT, "monitor_limit"));
        }
    }
    apply(&mut tx, project, &body).await?;
    let response = json!({"id":body.id,"version":body.expected_version+i64::from(body.action != Action::Check),"deleted":body.action == Action::Delete});
    crate::audit::record_in_transaction(
        &mut tx,
        Some(project),
        ctx.user_id,
        if body.action == Action::Check {
            "monitor.requested"
        } else if body.configuration.is_some() {
            "monitor.configured"
        } else {
            "monitor.deleted"
        },
        "monitor",
        &body.id.to_string(),
        json!({"version":response["version"]}),
    )
    .await
    .map_err(database_error)?;
    sqlx::query("INSERT INTO monitor_commands(project_id,actor_user_id,command_id,digest,response) VALUES($1,$2,$3,$4,$5)").bind(project).bind(ctx.user_id).bind(body.command_id).bind(digest).bind(&response).execute(&mut *tx).await.map_err(database_error)?;
    tx.commit().await.map_err(database_error)?;
    Ok(Json(response))
}

async fn apply(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    project: Uuid,
    body: &MonitorCommand,
) -> Result<(), Error> {
    if body.action == Action::Check {
        let result = sqlx::query("UPDATE monitors SET next_check_at=LEAST(next_check_at,now()) WHERE id=$1 AND project_id=$2 AND enabled AND deleted_at IS NULL")
            .bind(body.id).bind(project).execute(&mut **tx).await.map_err(database_error)?;
        if result.rows_affected() != 1 {
            return Err(error(StatusCode::CONFLICT, "monitor_disabled"));
        }
        return Ok(());
    }
    if let Some(c) = &body.configuration {
        // The project lock serializes the capacity and command checks; a globally reused ID
        // cannot overwrite a configuration in another project.
        let result=sqlx::query("INSERT INTO monitors(id,project_id,name,url,interval_seconds,timeout_seconds,expected_status,failure_threshold,enabled,generation) SELECT $1,$2,$3,$4,$5,$6,$7,$8,$9,generation FROM instance_settings WHERE singleton ON CONFLICT(id) DO UPDATE SET name=$3,url=$4,interval_seconds=$5,timeout_seconds=$6,expected_status=$7,failure_threshold=$8,enabled=$9,generation=EXCLUDED.generation,version=monitors.version+1,status='unknown',consecutive_failures=0,incident_id=NULL,lease_owner=NULL,lease_until=NULL,next_check_at=now(),updated_at=now() WHERE monitors.project_id=$2 AND monitors.version=$10 AND monitors.deleted_at IS NULL")
            .bind(body.id).bind(project).bind(c.name.trim()).bind(&c.url).bind(c.interval_seconds).bind(c.timeout_seconds).bind(c.expected_status).bind(c.failure_threshold).bind(c.enabled).bind(body.expected_version).execute(&mut **tx).await.map_err(database_error)?;
        if result.rows_affected() != 1 {
            return Err(error(StatusCode::CONFLICT, "monitor_changed"));
        }
    } else {
        sqlx::query("UPDATE monitors SET enabled=false,deleted_at=now(),version=version+1,lease_owner=NULL,lease_until=NULL,updated_at=now() WHERE id=$1 AND project_id=$2").bind(body.id).bind(project).execute(&mut **tx).await.map_err(database_error)?;
    }
    Ok(())
}
