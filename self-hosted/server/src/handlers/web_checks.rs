//! App-scoped, explicitly configured laboratory checks. Browser execution is external.
use super::{jobs::database_error, web_performance::require_web_app};
use crate::{session_mw::SessionContext, state::AppState};
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use sentori_ingest_token::IngestContext;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;
type Reply = (StatusCode, Json<Value>);
fn reject(status: StatusCode, code: &str) -> Reply {
    (status, Json(json!({"error":code})))
}
fn text(s: &str, n: usize) -> bool {
    !s.is_empty() && s.len() <= n && !s.chars().any(char::is_control)
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Step {
    action: String,
    selector: String,
    #[serde(default)]
    value: Option<String>,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Budgets {
    #[serde(default)]
    lcp_ms: Option<f64>,
    #[serde(default)]
    tbt_ms: Option<f64>,
    #[serde(default)]
    cls: Option<f64>,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Configuration {
    app_id: Uuid,
    name: String,
    url: String,
    policy: String,
    #[serde(default)]
    allowed_origins: Vec<String>,
    #[serde(default)]
    ready_selector: Option<String>,
    #[serde(default)]
    auth_profile: Option<String>,
    #[serde(default)]
    steps: Vec<Step>,
    #[serde(default)]
    interval_seconds: i32,
    #[serde(default)]
    budgets: Option<Budgets>,
    #[serde(default)]
    environment: String,
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    lighthouse: bool,
    #[serde(default = "default_device")]
    device: String,
    #[serde(default)]
    alerts: bool,
    #[serde(default = "default_threshold")]
    failure_threshold: i32,
}
fn default_device() -> String {
    "desktop".into()
}
fn default_threshold() -> i32 {
    2
}
impl Configuration {
    fn validate(&self) -> Result<(), Reply> {
        let url = crate::public_http::parse(&self.url)
            .map_err(|_| reject(StatusCode::BAD_REQUEST, "invalid_public_url"))?;
        if url.query().is_some()
            || !text(&self.name, 100)
            || !matches!(self.policy.as_str(), "public" | "private")
            || !matches!(self.device.as_str(), "mobile" | "desktop")
            || !(2..=10).contains(&self.failure_threshold)
            || self.allowed_origins.len() > 16
            || self.steps.len() > 10
            || self.environment.len() > 100
            || !(self.interval_seconds == 0 || (900..=604_800).contains(&self.interval_seconds))
            || self.ready_selector.as_ref().is_some_and(|s| !text(s, 200))
            || self.auth_profile.as_ref().is_some_and(|s| {
                !text(s, 64)
                    || !s
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
            })
            || self.steps.iter().any(|s| {
                !matches!(s.action.as_str(), "click" | "waitFor" | "assertText")
                    || !text(&s.selector, 200)
                    || s.value.as_ref().is_some_and(|v| !text(v, 200))
                    || s.action == "assertText" && s.value.is_none()
            })
        {
            return Err(reject(StatusCode::BAD_REQUEST, "invalid_web_check"));
        }
        for origin in &self.allowed_origins {
            let parsed = crate::public_http::parse(origin)
                .map_err(|_| reject(StatusCode::BAD_REQUEST, "invalid_allowed_origin"))?;
            if parsed.path() != "/" || parsed.query().is_some() {
                return Err(reject(StatusCode::BAD_REQUEST, "invalid_allowed_origin"));
            }
        }
        if let Some(b) = &self.budgets
            && ([b.lcp_ms, b.tbt_ms]
                .iter()
                .flatten()
                .any(|n| !n.is_finite() || !(0.0..=120_000.0).contains(n))
                || b.cls
                    .is_some_and(|n| !n.is_finite() || !(0.0..=10.0).contains(&n)))
        {
            return Err(reject(StatusCode::BAD_REQUEST, "invalid_lab_budget"));
        }
        Ok(())
    }
}
#[derive(Deserialize, Serialize)]
#[serde(tag = "action", rename_all = "camelCase", deny_unknown_fields)]
pub enum Command {
    Create {
        #[serde(rename = "commandId")]
        command_id: Uuid,
        configuration: Configuration,
    },
    Update {
        #[serde(rename = "commandId")]
        command_id: Uuid,
        id: Uuid,
        #[serde(rename = "expectedVersion")]
        expected_version: i64,
        configuration: Configuration,
    },
    Delete {
        #[serde(rename = "commandId")]
        command_id: Uuid,
        id: Uuid,
        #[serde(rename = "expectedVersion")]
        expected_version: i64,
    },
    Run {
        #[serde(rename = "commandId")]
        command_id: Uuid,
        id: Uuid,
        #[serde(default)]
        trigger: Option<String>,
    },
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Filter {
    app: Uuid,
    #[serde(default)]
    id: Option<Uuid>,
}
async fn list(state: &AppState, project: Uuid, filter: Filter) -> Result<Json<Value>, Reply> {
    require_web_app(state, project, filter.app).await?;
    let rows:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('id',c.id,'version',c.version,'configuration',c.configuration,'createdAt',c.created_at,'health',c.health,'consecutiveFailures',c.consecutive_failures,'incidentId',c.incident_id,'latestRun',(SELECT jsonb_build_object('id',r.id,'status',r.status,'at',r.created_at,'result',r.result) FROM web_check_runs r WHERE r.check_id=c.id AND r.project_id=$1 ORDER BY r.created_at DESC,r.id DESC LIMIT 1)) FROM web_checks c WHERE c.project_id=$1 AND c.app_id=$2 ORDER BY c.created_at,c.id LIMIT 50")
        .bind(project).bind(filter.app).fetch_all(&state.pool).await.map_err(database_error)?;
    let runner_configured =
        crate::env_config::env_or_file("SENTORI_WEB_RUNNER_TOKEN").is_some_and(|s| s.len() >= 32);
    Ok(Json(
        json!({"checks":rows,"source":"synthetic","runnerConfigured":runner_configured}),
    ))
}
async fn runs(state: &AppState, project: Uuid, filter: Filter) -> Result<Json<Value>, Reply> {
    require_web_app(state, project, filter.app).await?;
    let rows:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('id',id,'checkId',check_id,'appId',app_id,'configVersion',config_version,'status',status,'trigger',trigger,'createdAt',created_at,'startedAt',started_at,'finishedAt',finished_at,'configuration',configuration,'result',result) FROM web_check_runs WHERE project_id=$1 AND app_id=$2 AND ($3::uuid IS NULL OR id=$3) ORDER BY created_at DESC,id DESC LIMIT 50")
        .bind(project).bind(filter.app).bind(filter.id).fetch_all(&state.pool).await.map_err(database_error)?;
    if filter.id.is_some() && rows.is_empty() {
        return Err(reject(StatusCode::NOT_FOUND, "web_run_not_found"));
    }
    let days: i32 = sqlx::query_scalar("SELECT least(30,coalesce((SELECT (limits->>'eventRetentionDays')::int FROM cloud_entitlement WHERE singleton),30))").fetch_one(&state.pool).await.map_err(database_error)?;
    Ok(Json(
        json!({"runs":rows,"source":"synthetic","retentionDays":days}),
    ))
}
async fn apply_command(
    connection: &mut sqlx::PgConnection,
    project: Uuid,
    body: Command,
) -> Result<Value, Reply> {
    let result = match body {
        Command::Create {
            configuration: c, ..
        } => {
            let count: i64 =
                sqlx::query_scalar("SELECT count(*) FROM web_checks WHERE project_id=$1")
                    .bind(project)
                    .fetch_one(&mut *connection)
                    .await
                    .map_err(database_error)?;
            if count >= 50 {
                return Err(reject(StatusCode::CONFLICT, "web_check_capacity"));
            }
            let id = Uuid::now_v7();
            sqlx::query("INSERT INTO web_checks(id,project_id,app_id,configuration,enabled,interval_seconds) VALUES($1,$2,$3,$4,$5,$6)")
                .bind(id).bind(project).bind(c.app_id).bind(json!(c)).bind(c.enabled).bind(c.interval_seconds).execute(&mut *connection).await.map_err(database_error)?;
            json!({"id":id,"version":1})
        }
        Command::Update {
            id,
            expected_version,
            configuration: c,
            ..
        } => {
            let version:Option<i64>=sqlx::query_scalar("UPDATE web_checks SET app_id=$4,configuration=$5,enabled=$6,interval_seconds=$7,version=version+1,next_run_at=now(),consecutive_failures=0,incident_id=NULL,health='unknown' WHERE id=$1 AND project_id=$2 AND version=$3 RETURNING version")
                .bind(id).bind(project).bind(expected_version).bind(c.app_id).bind(json!(c)).bind(c.enabled).bind(c.interval_seconds).fetch_optional(&mut *connection).await.map_err(database_error)?;
            let version =
                version.ok_or_else(|| reject(StatusCode::CONFLICT, "web_check_conflict"))?;
            sqlx::query("UPDATE web_check_runs SET status='cancelled',finished_at=now() WHERE project_id=$1 AND check_id=$2 AND status IN ('queued','running')").bind(project).bind(id).execute(&mut *connection).await.map_err(database_error)?;
            json!({"id":id,"version":version})
        }
        Command::Delete {
            id,
            expected_version,
            ..
        } => {
            let result =
                sqlx::query("DELETE FROM web_checks WHERE id=$1 AND project_id=$2 AND version=$3")
                    .bind(id)
                    .bind(project)
                    .bind(expected_version)
                    .execute(&mut *connection)
                    .await
                    .map_err(database_error)?;
            if result.rows_affected() != 1 {
                return Err(reject(StatusCode::CONFLICT, "web_check_conflict"));
            }
            json!({"id":id,"deleted":true})
        }
        Command::Run { id, trigger, .. } => {
            let trigger = trigger.unwrap_or_else(|| "manual".into());
            if !matches!(trigger.as_str(), "manual" | "deployment") {
                return Err(reject(StatusCode::BAD_REQUEST, "invalid_trigger"));
            }
            if crate::env_config::env_or_file("SENTORI_WEB_RUNNER_TOKEN")
                .is_none_or(|s| s.len() < 32)
            {
                return Err(reject(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "web_runner_not_configured",
                ));
            }
            let count:i64=sqlx::query_scalar("SELECT count(*) FROM web_check_runs WHERE project_id=$1 AND status IN ('queued','running')").bind(project).fetch_one(&mut *connection).await.map_err(database_error)?;
            if count >= 2 {
                return Err(reject(
                    StatusCode::TOO_MANY_REQUESTS,
                    "web_check_concurrency",
                ));
            }
            let run = Uuid::now_v7();
            let result=sqlx::query("INSERT INTO web_check_runs(id,project_id,check_id,app_id,config_version,configuration,generation,trigger) SELECT $3,c.project_id,c.id,c.app_id,c.version,c.configuration,s.generation,$4 FROM web_checks c CROSS JOIN instance_settings s WHERE c.id=$1 AND c.project_id=$2 AND s.singleton AND s.operating_mode='normal'")
                .bind(id).bind(project).bind(run).bind(trigger).execute(&mut *connection).await.map_err(database_error)?;
            if result.rows_affected() != 1 {
                return Err(reject(
                    StatusCode::NOT_FOUND,
                    "web_check_not_found_or_instance_paused",
                ));
            }
            json!({"id":run,"checkId":id,"status":"queued"})
        }
    };
    Ok(result)
}

async fn command(
    state: &AppState,
    project: Uuid,
    body: Command,
    actor: Option<Uuid>,
    token: Option<Uuid>,
) -> Result<Json<Value>, Reply> {
    let command_id = match &body {
        Command::Create { command_id, .. }
        | Command::Update { command_id, .. }
        | Command::Delete { command_id, .. }
        | Command::Run { command_id, .. } => *command_id,
    };
    let request_hash =
        hex::encode(Sha256::digest(serde_json::to_vec(&body).map_err(|_| {
            reject(StatusCode::BAD_REQUEST, "invalid_web_check")
        })?));
    let action = match &body {
        Command::Create { .. } => "web_check.create",
        Command::Update { .. } => "web_check.update",
        Command::Delete { .. } => "web_check.delete",
        Command::Run { .. } => "web_check.run",
    };
    let config = match &body {
        Command::Create { configuration, .. } | Command::Update { configuration, .. } => {
            Some(configuration)
        }
        _ => None,
    };
    if let Some(c) = config {
        c.validate()?;
        require_web_app(state, project, c.app_id).await?;
    }
    let mut tx = state.pool.begin().await.map_err(database_error)?;
    sqlx::query("SELECT id FROM projects WHERE id=$1 AND migration_fence IS NULL FOR UPDATE")
        .bind(project)
        .fetch_optional(&mut *tx)
        .await
        .map_err(database_error)?
        .ok_or_else(|| reject(StatusCode::CONFLICT, "project_unavailable"))?;
    let prior: Option<(String, Value)> = sqlx::query_as(
        "SELECT request_hash,result FROM web_check_commands WHERE project_id=$1 AND id=$2",
    )
    .bind(project)
    .bind(command_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(database_error)?;
    if let Some((hash, result)) = prior {
        return if hash == request_hash {
            Ok(Json(result))
        } else {
            Err(reject(StatusCode::CONFLICT, "web_command_conflict"))
        };
    }
    let result = apply_command(&mut tx, project, body).await?;
    sqlx::query("INSERT INTO audit_logs(id,project_id,actor_user_id,action,target_type,target_id,payload,ip) VALUES($1,$2,$3,$4,'web_check',$5,$6,$7)")
        .bind(Uuid::now_v7()).bind(project).bind(actor).bind(action).bind(result["id"].as_str().unwrap_or("")).bind(json!({"tokenId":token,"result":result})).bind(sentori_request_origin::current()).execute(&mut *tx).await.map_err(database_error)?;
    sqlx::query(
        "INSERT INTO web_check_commands(project_id,id,request_hash,result) VALUES($1,$2,$3,$4)",
    )
    .bind(project)
    .bind(command_id)
    .bind(request_hash)
    .bind(&result)
    .execute(&mut *tx)
    .await
    .map_err(database_error)?;
    tx.commit().await.map_err(database_error)?;
    Ok(Json(result))
}
pub async fn api_list(
    State(s): State<Arc<AppState>>,
    Extension(c): Extension<IngestContext>,
    Query(f): Query<Filter>,
) -> Result<Json<Value>, Reply> {
    super::sdk::require_admin_token(&c)?;
    list(&s, c.project_id, f).await
}
pub async fn api_runs(
    State(s): State<Arc<AppState>>,
    Extension(c): Extension<IngestContext>,
    Query(f): Query<Filter>,
) -> Result<Json<Value>, Reply> {
    super::sdk::require_admin_token(&c)?;
    runs(&s, c.project_id, f).await
}
pub async fn api_command(
    State(s): State<Arc<AppState>>,
    Extension(c): Extension<IngestContext>,
    Json(b): Json<Command>,
) -> Result<Json<Value>, Reply> {
    super::sdk::require_admin_token(&c)?;
    command(&s, c.project_id, b, None, Some(c.token_id)).await
}
pub async fn admin_list(
    State(s): State<Arc<AppState>>,
    Extension(c): Extension<SessionContext>,
    Path(p): Path<Uuid>,
    Query(f): Query<Filter>,
) -> Result<Json<Value>, Reply> {
    super::admin::tokens::ensure_project_access(&s, &c, p).await?;
    list(&s, p, f).await
}
pub async fn admin_runs(
    State(s): State<Arc<AppState>>,
    Extension(c): Extension<SessionContext>,
    Path(p): Path<Uuid>,
    Query(f): Query<Filter>,
) -> Result<Json<Value>, Reply> {
    super::admin::tokens::ensure_project_access(&s, &c, p).await?;
    runs(&s, p, f).await
}
pub async fn admin_command(
    State(s): State<Arc<AppState>>,
    Extension(c): Extension<SessionContext>,
    Path(p): Path<Uuid>,
    Json(b): Json<Command>,
) -> Result<Json<Value>, Reply> {
    super::admin::tokens::ensure_project_access(&s, &c, p).await?;
    if !c.role.is_superadmin() {
        return Err(reject(StatusCode::FORBIDDEN, "superadmin_only"));
    }
    command(&s, p, b, Some(c.user_id), None).await
}

fn worker_auth(headers: &HeaderMap) -> Result<(), Reply> {
    let expected = crate::env_config::env_or_file("SENTORI_WEB_RUNNER_TOKEN").unwrap_or_default();
    let actual = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    if expected.len() < 32
        || !sentori_cookie_session::constant_time_eq(expected.as_bytes(), actual.as_bytes())
    {
        return Err(reject(StatusCode::UNAUTHORIZED, "invalid_runner_token"));
    }
    Ok(())
}
pub async fn claim(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Value>, Reply> {
    worker_auth(&headers)?;
    let mut tx = state.pool.begin().await.map_err(database_error)?;
    sqlx::query("SELECT singleton FROM instance_settings WHERE singleton FOR UPDATE")
        .fetch_one(&mut *tx)
        .await
        .map_err(database_error)?;
    let running: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM web_check_runs WHERE status='running' AND lease_until>now()",
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(database_error)?;
    if running >= 2 {
        return Ok(Json(json!({"job":null})));
    }
    // Acquire project before run rows, matching commands and transfer guards.
    let project: Option<Uuid> = sqlx::query_scalar("SELECT p.id FROM projects p JOIN web_check_runs r ON r.project_id=p.id WHERE r.status='queued' AND p.migration_fence IS NULL ORDER BY r.created_at,r.id LIMIT 1 FOR UPDATE OF p SKIP LOCKED").fetch_optional(&mut *tx).await.map_err(database_error)?;
    let Some(project) = project else {
        return Ok(Json(json!({"job":null})));
    };
    let lease = Uuid::new_v4();
    let row=sqlx::query("WITH due AS (SELECT r.id FROM web_check_runs r JOIN web_checks c ON c.id=r.check_id AND c.version=r.config_version JOIN projects p ON p.id=r.project_id CROSS JOIN instance_settings s WHERE r.project_id=$2 AND r.status='queued' AND r.generation=s.generation AND s.singleton AND s.operating_mode='normal' AND p.migration_fence IS NULL AND NOT EXISTS(SELECT 1 FROM cloud_entitlement WHERE singleton AND (NOT active OR valid_until<=now())) ORDER BY r.created_at,r.id LIMIT 1 FOR UPDATE OF r SKIP LOCKED) UPDATE web_check_runs r SET status='running',lease_id=$1,lease_until=now()+interval '300 seconds',started_at=now() FROM due WHERE r.id=due.id RETURNING r.id,r.project_id,r.app_id,r.configuration")
        .bind(lease).bind(project).fetch_optional(&mut *tx).await.map_err(database_error)?;
    tx.commit().await.map_err(database_error)?;
    Ok(Json(match row {
        Some(row) => {
            json!({"job":{"id":row.get::<Uuid,_>("id"),"leaseId":lease,"projectId":row.get::<Uuid,_>("project_id"),"appId":row.get::<Uuid,_>("app_id"),"configuration":row.get::<Value,_>("configuration")}})
        }
        None => json!({"job":null}),
    }))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Completion {
    lease_id: Uuid,
    result: Value,
}
pub async fn complete(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<Completion>,
) -> Result<Json<Value>, Reply> {
    worker_auth(&headers)?;
    validate_result(&body.result)?;
    let status = body
        .result
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("");
    if !matches!(status, "passed" | "failed" | "error")
        || body.result.get("source").and_then(Value::as_str) != Some("synthetic")
        || body.result.get("ruleVersion").and_then(Value::as_u64) != Some(1)
        || body
            .result
            .get("findings")
            .and_then(Value::as_array)
            .is_none_or(|f| f.len() > 100)
        || serde_json::to_vec(&body.result).map_or(true, |v| v.len() > 128 * 1024)
    {
        return Err(reject(StatusCode::BAD_REQUEST, "invalid_web_result"));
    }
    let mut tx = state.pool.begin().await.map_err(database_error)?;
    sqlx::query("SELECT p.id FROM projects p JOIN web_check_runs r ON r.project_id=p.id WHERE r.id=$1 AND p.migration_fence IS NULL FOR UPDATE OF p").bind(id).fetch_optional(&mut *tx).await.map_err(database_error)?.ok_or_else(||reject(StatusCode::CONFLICT,"web_run_lease_expired"))?;
    let result=sqlx::query("UPDATE web_check_runs r SET status=$3,result=$4,finished_at=now(),lease_until=NULL FROM instance_settings s,web_checks c,projects p WHERE p.id=r.project_id AND p.migration_fence IS NULL AND r.id=$1 AND r.lease_id=$2 AND r.status='running' AND r.lease_until>now() AND r.generation=s.generation AND s.singleton AND s.operating_mode='normal' AND c.id=r.check_id AND c.version=r.config_version AND NOT EXISTS(SELECT 1 FROM cloud_entitlement WHERE singleton AND (NOT active OR valid_until<=now()))")
        .bind(id).bind(body.lease_id).bind(status).bind(&body.result).execute(&mut *tx).await.map_err(database_error)?;
    if result.rows_affected() != 1 {
        return Err(reject(StatusCode::CONFLICT, "web_run_lease_expired"));
    }
    let check = sqlx::query("SELECT c.* FROM web_checks c JOIN web_check_runs r ON r.check_id=c.id WHERE r.id=$1 FOR UPDATE OF c").bind(id).fetch_one(&mut *tx).await.map_err(database_error)?;
    let previous: Option<Uuid> = check.get("incident_id");
    // Two workers may finish in reverse order. Derive health from run order,
    // not completion order, so an old result cannot overwrite a newer recovery.
    let recent: Vec<String> = sqlx::query_scalar("SELECT status FROM web_check_runs WHERE check_id=$1 AND config_version=$2 AND status IN ('passed','failed','error') ORDER BY created_at DESC,id DESC LIMIT 10").bind(check.get::<Uuid,_>("id")).bind(check.get::<i64,_>("version")).fetch_all(&mut *tx).await.map_err(database_error)?;
    let health_status = recent.first().map_or(status, String::as_str);
    let failures =
        i32::try_from(recent.iter().take_while(|s| s.as_str() == "failed").count()).unwrap_or(10);
    let config: Configuration =
        serde_json::from_value(check.get("configuration")).map_err(|_| {
            reject(
                StatusCode::INTERNAL_SERVER_ERROR,
                "invalid_stored_web_check",
            )
        })?;
    let health = if health_status == "error" {
        "monitor-error"
    } else if health_status == "passed" {
        "healthy"
    } else if failures >= config.failure_threshold {
        "failing"
    } else {
        "degraded"
    };
    let incident = if health == "failing" {
        Some(previous.unwrap_or_else(Uuid::now_v7))
    } else if health_status == "passed" {
        None
    } else {
        previous
    };
    sqlx::query(
        "UPDATE web_checks SET consecutive_failures=$2,health=$3,incident_id=$4 WHERE id=$1",
    )
    .bind(check.get::<Uuid, _>("id"))
    .bind(failures)
    .bind(health)
    .bind(incident)
    .execute(&mut *tx)
    .await
    .map_err(database_error)?;
    if config.alerts
        && ((previous.is_none() && incident.is_some())
            || (previous.is_some() && health_status == "passed"))
    {
        let project: Uuid = check.get("project_id");
        let app: Uuid = check.get("app_id");
        let kind = if health_status == "passed" {
            "monitor.recovered"
        } else {
            "monitor.down"
        };
        crate::notification_queue::enqueue(&mut tx, crate::notification_queue::Event {
            project, key: format!("web:{id}:{kind}"),kind: kind.into(),payload: json!({"event":kind,"source":"synthetic","title":config.name,"body":health,"route":format!("/web?project={project}&app={app}&tab=checks&run={id}")}),issue:None,actor:None,target:None,
        }).await.map_err(database_error)?;
    }
    tx.commit().await.map_err(database_error)?;
    Ok(Json(json!({"id":id,"status":status})))
}

fn validate_result(value: &Value) -> Result<(), Reply> {
    let fail = || reject(StatusCode::BAD_REQUEST, "invalid_web_result");
    let object = value.as_object().ok_or_else(fail)?;
    if object.keys().any(|k| {
        !matches!(
            k.as_str(),
            "source"
                | "status"
                | "ruleVersion"
                | "checkedAt"
                | "url"
                | "profile"
                | "versions"
                | "metrics"
                | "findings"
                | "coverage"
        )
    }) {
        return Err(fail());
    }
    for key in ["url", "checkedAt"] {
        if !object
            .get(key)
            .and_then(Value::as_str)
            .is_some_and(|s| text(s, 500))
        {
            return Err(fail());
        }
    }
    for key in ["profile", "versions", "coverage"] {
        let map = object
            .get(key)
            .and_then(Value::as_object)
            .ok_or_else(fail)?;
        if map.len() > 12
            || map.iter().any(|(k, v)| {
                !text(k, 50)
                    || !(v.as_str().is_some_and(|s| text(s, 200))
                        || v.as_u64().is_some_and(|n| n <= 100_000)
                        || v.is_boolean())
            })
        {
            return Err(fail());
        }
    }
    let metrics = object
        .get("metrics")
        .and_then(Value::as_array)
        .ok_or_else(fail)?;
    if metrics.len() > 10
        || metrics.iter().any(|m| {
            m.as_object().is_none_or(|o| o.len() != 4)
                || !m["name"].as_str().is_some_and(|s| {
                    matches!(s, "LCP" | "TBT" | "CLS" | "FCP" | "TTFB" | "navigation")
                })
                || !m["value"]
                    .as_f64()
                    .is_some_and(|v| v.is_finite() && (0.0..=86_400_000.0).contains(&v))
                || !m["unit"]
                    .as_str()
                    .is_some_and(|s| matches!(s, "ms" | "score"))
                || !m["source"]
                    .as_str()
                    .is_some_and(|s| matches!(s, "lighthouse" | "browser"))
        })
    {
        return Err(fail());
    }
    let findings = object
        .get("findings")
        .and_then(Value::as_array)
        .ok_or_else(fail)?;
    if findings.len() > 100
        || findings.iter().any(|f| {
            f.as_object().is_none_or(|o| o.len() != 5)
                || !f["rule"].as_str().is_some_and(|s| text(s, 80))
                || !f["severity"]
                    .as_str()
                    .is_some_and(|s| matches!(s, "error" | "warning" | "info"))
                || !f["evidence"].as_str().is_some_and(|s| text(s, 500))
                || !f["advice"].as_str().is_some_and(|s| text(s, 500))
                || !f["url"].as_str().is_some_and(|s| {
                    text(s, 500)
                        && url::Url::parse(s).is_ok_and(|u| {
                            matches!(u.scheme(), "http" | "https")
                                && u.username().is_empty()
                                && u.password().is_none()
                                && u.query().is_none()
                                && u.fragment().is_none()
                        })
                })
        })
    {
        return Err(fail());
    }
    let has_errors = findings.iter().any(|f| f["severity"] == "error");
    if (value["status"] == "passed" && has_errors) || (value["status"] == "failed" && !has_errors) {
        return Err(fail());
    }
    Ok(())
}

pub fn spawn(state: Arc<AppState>) {
    tokio::spawn(async move {
        loop {
            if let Err(error) = maintain(&state).await {
                tracing::warn!(%error,"web observation maintenance failed");
            }
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        }
    });
}
async fn maintain(state: &AppState) -> Result<(), sqlx::Error> {
    let mut tx = state.pool.begin().await?;
    // A single bounded maintenance pass across replicas; do not queue a lock waiter.
    let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(83157001)")
        .fetch_one(&mut *tx)
        .await?;
    if !locked {
        return Ok(());
    }
    let days: i32 = sqlx::query_scalar("SELECT least(30,coalesce((SELECT (limits->>'eventRetentionDays')::int FROM cloud_entitlement WHERE singleton),30))").fetch_one(&mut *tx).await?;
    let projects: Vec<Uuid> = sqlx::query_scalar("SELECT p.id FROM projects p WHERE p.migration_fence IS NULL AND (EXISTS(SELECT 1 FROM web_check_runs r WHERE r.project_id=p.id AND ((r.status='running' AND r.lease_until<now()) OR (r.status='queued' AND r.created_at<now()-interval '10 minutes') OR r.finished_at<now()-$1*interval '1 day')) OR EXISTS(SELECT 1 FROM web_visits v WHERE v.project_id=p.id AND v.received_at<now()-$1*interval '1 day') OR EXISTS(SELECT 1 FROM web_check_commands c WHERE c.project_id=p.id AND c.created_at<now()-interval '30 days') OR EXISTS(SELECT 1 FROM web_checks c WHERE c.project_id=p.id AND c.enabled AND c.interval_seconds>0 AND c.next_run_at<=now())) ORDER BY p.id LIMIT 50 FOR UPDATE OF p SKIP LOCKED").bind(days).fetch_all(&mut *tx).await?;
    if projects.is_empty() {
        return Ok(());
    }
    sqlx::query("WITH expired AS (UPDATE web_check_runs r SET status='error',lease_id=NULL,lease_until=NULL,finished_at=now(),result=jsonb_build_object('source','synthetic','status','error','ruleVersion',1,'errorCode','runner_timeout') WHERE r.project_id=ANY($1) AND ((r.status='running' AND r.lease_until<now()) OR (r.status='queued' AND r.created_at<now()-interval '10 minutes')) RETURNING r.check_id,r.config_version,r.created_at,r.id) UPDATE web_checks c SET health='monitor-error',consecutive_failures=0 FROM expired e WHERE c.id=e.check_id AND c.version=e.config_version AND NOT EXISTS(SELECT 1 FROM web_check_runs newer WHERE newer.check_id=c.id AND newer.config_version=c.version AND (newer.created_at,newer.id)>(e.created_at,e.id) AND newer.status IN ('passed','failed','error'))").bind(&projects).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM web_visits WHERE (project_id,id) IN (SELECT v.project_id,v.id FROM web_visits v JOIN projects p ON p.id=v.project_id WHERE v.project_id=ANY($2) AND p.migration_fence IS NULL AND v.received_at<now()-$1*interval '1 day' ORDER BY v.received_at LIMIT 1000)").bind(days).bind(&projects).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM web_check_runs WHERE id IN (SELECT r.id FROM web_check_runs r JOIN projects p ON p.id=r.project_id WHERE r.project_id=ANY($2) AND p.migration_fence IS NULL AND r.finished_at<now()-$1*interval '1 day' ORDER BY r.finished_at LIMIT 1000)").bind(days).bind(&projects).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM web_check_commands WHERE (project_id,id) IN (SELECT c.project_id,c.id FROM web_check_commands c JOIN projects p ON p.id=c.project_id WHERE c.project_id=ANY($1) AND p.migration_fence IS NULL AND c.created_at<now()-interval '30 days' ORDER BY c.created_at LIMIT 1000)").bind(&projects).execute(&mut *tx).await?;
    if crate::env_config::env_or_file("SENTORI_WEB_RUNNER_TOKEN").is_some_and(|s| s.len() >= 32) {
        let due=sqlx::query("SELECT c.* FROM web_checks c JOIN projects p ON p.id=c.project_id CROSS JOIN instance_settings s WHERE c.project_id=ANY($1) AND c.enabled AND c.interval_seconds>0 AND c.next_run_at<=now() AND p.migration_fence IS NULL AND s.singleton AND s.operating_mode='normal' AND NOT EXISTS(SELECT 1 FROM cloud_entitlement WHERE singleton AND (NOT active OR valid_until<=now())) ORDER BY c.next_run_at LIMIT 50").bind(&projects).fetch_all(&mut *tx).await?;
        for row in due {
            let project: Uuid = row.get("project_id");
            sqlx::query(
                "SELECT id FROM projects WHERE id=$1 AND migration_fence IS NULL FOR UPDATE",
            )
            .bind(project)
            .fetch_one(&mut *tx)
            .await?;
            let pending: i64 = sqlx::query_scalar("SELECT count(*) FROM web_check_runs WHERE project_id=$1 AND status IN ('queued','running')").bind(project).fetch_one(&mut *tx).await?;
            if pending >= 2 {
                continue;
            }
            sqlx::query("INSERT INTO web_check_runs(project_id,check_id,app_id,config_version,configuration,generation,trigger) SELECT c.project_id,c.id,c.app_id,c.version,c.configuration,s.generation,'schedule' FROM web_checks c CROSS JOIN instance_settings s WHERE c.id=$1 AND c.enabled AND c.interval_seconds>0 AND c.next_run_at<=now() AND s.singleton").bind(row.get::<Uuid,_>("id")).execute(&mut *tx).await?;
            sqlx::query("UPDATE web_checks SET next_run_at=now()+interval_seconds*interval '1 second' WHERE id=$1").bind(row.get::<Uuid,_>("id")).execute(&mut *tx).await?;
        }
    }
    tx.commit().await
}
