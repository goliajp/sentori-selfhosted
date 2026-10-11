//! Typed, versioned work-item commands. Commands, history, audit and outbox commit together.
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
use sqlx::{Postgres, Row, Transaction};
use std::sync::Arc;
use uuid::Uuid;
type Error = (StatusCode, Json<Value>);
fn error(status: StatusCode, code: &str) -> Error {
    (status, Json(json!({"error":code})))
}
const FIELDS: &str = "jsonb_build_object('id',t.id,'projectId',t.project_id,'issueId',t.issue_id,'assignee',t.assignee_user_id,'title',t.title,'description',t.description,'kind',t.kind,'priority',t.priority,'stage',t.stage,'due',t.due_date,'reproduction',t.reproduction,'rootCause',t.root_cause,'fixRelease',t.fix_release,'verified',t.verified,'automation',t.automation,'version',t.version,'createdAt',t.created_at,'updatedAt',t.updated_at)";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct List {
    q: Option<String>,
    issue_id: Option<Uuid>,
    stage: Option<String>,
    mine: Option<bool>,
    offset: Option<i64>,
    limit: Option<i64>,
}
pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project): Path<Uuid>,
    Query(q): Query<List>,
) -> Result<Json<Value>, Error> {
    super::admin::tokens::ensure_project_access(&state, &ctx, project).await?;
    let offset = q.offset.unwrap_or(0).clamp(0, 1_000_000);
    let limit = q.limit.unwrap_or(50).clamp(1, 100);
    let query = format!(
        "WITH selected AS MATERIALIZED (SELECT t.* FROM tasks t WHERE project_id=$1 AND ($2::text IS NULL OR strpos(lower(title),lower($2))>0) AND ($3::text IS NULL OR stage=$3) AND (NOT $4 OR assignee_user_id=$5) AND ($8::uuid IS NULL OR issue_id=$8)), page AS (SELECT {FIELDS} AS item FROM selected t ORDER BY updated_at DESC,id DESC LIMIT $6 OFFSET $7) SELECT (SELECT count(*) FROM selected) AS total,(SELECT jsonb_build_object('active',count(*) FILTER (WHERE stage NOT IN ('done','cancelled')),'review',count(*) FILTER (WHERE stage IN ('review','verify')),'done',count(*) FILTER (WHERE stage='done'),'overdue',count(*) FILTER (WHERE due_date<current_date AND stage NOT IN ('done','cancelled'))) FROM selected) AS summary,COALESCE((SELECT jsonb_agg(item) FROM page),'[]'::jsonb) AS tasks"
    );
    let row = sqlx::query(sqlx::AssertSqlSafe(query))
        .bind(project)
        .bind(q.q)
        .bind(q.stage)
        .bind(q.mine.unwrap_or(false))
        .bind(ctx.user_id)
        .bind(limit)
        .bind(offset)
        .bind(q.issue_id)
        .fetch_one(&state.pool)
        .await
        .map_err(database_error)?;
    let total: i64 = row.get("total");
    let members:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('id',u.id,'name',COALESCE(u.display_name,u.email)) FROM users u WHERE u.role='superadmin' OR EXISTS(SELECT 1 FROM project_assignments a WHERE a.project_id=$1 AND a.user_id=u.id) ORDER BY u.id").bind(project).fetch_all(&state.pool).await.map_err(database_error)?;

    Ok(Json(
        json!({"tasks":row.get::<Value,_>("tasks"),"members":members,"total":total,"summary":row.get::<Value,_>("summary"),"nextOffset":(offset+limit<total).then_some(offset+limit)}),
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Page {
    offset: Option<i64>,
}
pub async fn get(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path((project, id)): Path<(Uuid, Uuid)>,
    Query(q): Query<Page>,
) -> Result<Json<Value>, Error> {
    super::admin::tokens::ensure_project_access(&state, &ctx, project).await?;
    let task: Value = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT {FIELDS} FROM tasks t WHERE project_id=$1 AND id=$2"
    )))
    .bind(project)
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(database_error)?
    .ok_or_else(|| error(StatusCode::NOT_FOUND, "not_found"))?;
    let offset = q.offset.unwrap_or(0).clamp(0, 1_000_000);
    let rows:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('id',e.id,'actor',e.actor_user_id,'actorName',u.email,'action',e.action,'body',e.body,'at',e.created_at) FROM task_events e LEFT JOIN users u ON u.id=e.actor_user_id WHERE e.task_id=$1 ORDER BY e.created_at DESC,e.id DESC LIMIT 51 OFFSET $2").bind(id).bind(offset).fetch_all(&state.pool).await.map_err(database_error)?;
    Ok(Json(
        json!({"task":task,"events":rows.iter().take(50).collect::<Vec<_>>(),"nextOffset":(rows.len()>50).then_some(offset+50)}),
    ))
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Fields {
    title: String,
    description: String,
    kind: String,
    priority: String,
    assignee: Option<Uuid>,
    due: Option<String>,
    reproduction: String,
    root_cause: String,
}
impl Fields {
    fn validate(&mut self) -> Result<(), Error> {
        self.title = self.title.trim().to_owned();
        if self.title.is_empty()
            || self.title.chars().count() > 200
            || !matches!(self.kind.as_str(), "task" | "bug")
            || !matches!(self.priority.as_str(), "urgent" | "high" | "normal" | "low")
            || [&self.description, &self.reproduction, &self.root_cause]
                .iter()
                .any(|s| s.len() > 16000)
        {
            return Err(error(StatusCode::BAD_REQUEST, "invalid_task"));
        }
        if let Some(due) = &self.due {
            time::Date::parse(
                due,
                &time::macros::format_description!("[year]-[month]-[day]"),
            )
            .map_err(|_| error(StatusCode::BAD_REQUEST, "invalid_due"))?;
        }
        Ok(())
    }
}
#[derive(Deserialize, Serialize)]
#[serde(
    tag = "action",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Command {
    Create {
        command_id: Uuid,
        issue_id: Option<Uuid>,
        fields: Fields,
    },
    Edit {
        command_id: Uuid,
        task_id: Uuid,
        expected_version: i64,
        fields: Fields,
    },
    Transition {
        command_id: Uuid,
        task_id: Uuid,
        expected_version: i64,
        stage: String,
    },
    Comment {
        command_id: Uuid,
        task_id: Uuid,
        expected_version: i64,
        body: String,
        mention: Option<Uuid>,
    },
}
impl Command {
    fn identity(&self) -> Result<(Uuid, String), Error> {
        let bytes =
            serde_json::to_vec(self).map_err(|_| error(StatusCode::BAD_REQUEST, "invalid_task"))?;
        Ok((self.key(), hex::encode(Sha256::digest(bytes))))
    }
    fn key(&self) -> Uuid {
        match self {
            Self::Create { command_id, .. }
            | Self::Edit { command_id, .. }
            | Self::Transition { command_id, .. }
            | Self::Comment { command_id, .. } => *command_id,
        }
    }
    fn target(&self) -> Option<(Uuid, i64)> {
        match self {
            Self::Create { .. } => None,
            Self::Edit {
                task_id,
                expected_version,
                ..
            }
            | Self::Transition {
                task_id,
                expected_version,
                ..
            }
            | Self::Comment {
                task_id,
                expected_version,
                ..
            } => Some((*task_id, *expected_version)),
        }
    }
}
async fn member(
    tx: &mut Transaction<'_, Postgres>,
    project: Uuid,
    user: Option<Uuid>,
) -> Result<(), Error> {
    if let Some(user) = user {
        let valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users u WHERE u.id=$1 AND (u.role='superadmin' OR EXISTS(SELECT 1 FROM project_assignments a WHERE a.user_id=u.id AND a.project_id=$2)))").bind(user).bind(project).fetch_one(&mut **tx).await.map_err(database_error)?;
        if !valid {
            return Err(error(StatusCode::BAD_REQUEST, "invalid_member"));
        }
    }
    Ok(())
}
fn allowed(from: &str, to: &str) -> bool {
    match from {
        "triage" => matches!(to, "ready" | "cancelled"),
        "ready" => matches!(to, "inProgress" | "triage" | "cancelled"),
        "inProgress" => matches!(to, "review" | "ready" | "cancelled"),
        "review" => matches!(to, "verify" | "inProgress" | "cancelled"),
        "verify" => matches!(to, "done" | "inProgress" | "cancelled"),
        "done" => to == "inProgress",
        "cancelled" => to == "triage",
        _ => false,
    }
}
pub async fn command(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project): Path<Uuid>,
    Json(mut cmd): Json<Command>,
) -> Result<Json<Value>, Error> {
    super::admin::tokens::ensure_project_access(&state, &ctx, project).await?;
    let (key, digest) = cmd.identity()?;
    let mut tx = state.pool.begin().await.map_err(database_error)?;
    // Serialize commands and their dedup check per project. No network work is done under this lock.
    sqlx::query("SELECT id FROM projects WHERE id=$1 FOR NO KEY UPDATE")
        .bind(project)
        .fetch_optional(&mut *tx)
        .await
        .map_err(database_error)?
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "not_found"))?;
    member(&mut tx, project, Some(ctx.user_id)).await?;
    if let Some(row)=sqlx::query("SELECT digest,response FROM task_commands WHERE project_id=$1 AND actor_user_id=$2 AND command_id=$3").bind(project).bind(ctx.user_id).bind(key).fetch_optional(&mut *tx).await.map_err(database_error)? {
        if row.get::<String,_>("digest")!=digest {return Err(error(StatusCode::CONFLICT,"idempotency_conflict"));}
        return Ok(Json(row.get("response")));
    }
    let target = cmd.target();
    // Issue locks precede task locks, matching ingestion's regression path.
    let issue: Option<Uuid> = if let Command::Create { issue_id, .. } = &cmd {
        *issue_id
    } else {
        sqlx::query_scalar("SELECT issue_id FROM tasks WHERE id=$1 AND project_id=$2")
            .bind(target.map(|v| v.0))
            .bind(project)
            .fetch_optional(&mut *tx)
            .await
            .map_err(database_error)?
            .ok_or_else(|| error(StatusCode::NOT_FOUND, "not_found"))?
    };
    if let Some(issue) = issue {
        sqlx::query("SELECT id FROM issues WHERE id=$1 AND project_id=$2 FOR UPDATE")
            .bind(issue)
            .bind(project)
            .fetch_optional(&mut *tx)
            .await
            .map_err(database_error)?
            .ok_or_else(|| error(StatusCode::BAD_REQUEST, "invalid_issue"))?;
    }
    let current = if let Some((id, version)) = target {
        let row = sqlx::query("SELECT * FROM tasks WHERE id=$1 AND project_id=$2 FOR UPDATE")
            .bind(id)
            .bind(project)
            .fetch_optional(&mut *tx)
            .await
            .map_err(database_error)?
            .ok_or_else(|| error(StatusCode::NOT_FOUND, "not_found"))?;
        if row.get::<i64, _>("version") != version {
            return Err(error(StatusCode::CONFLICT, "task_changed"));
        }
        Some(row)
    } else {
        None
    };
    let id = target.map_or_else(Uuid::now_v7, |v| v.0);
    let snapshot = current.as_ref().map(|r| crate::team::TaskSnapshot {
        stage: r.get("stage"),
        assignee: r.get("assignee_user_id"),
    });
    let (action, body, recipient, category) = apply(
        &mut tx,
        project,
        ctx.user_id,
        &mut cmd,
        snapshot.as_ref(),
        id,
        issue,
    )
    .await?;
    crate::team::event(
        &mut tx,
        id,
        Some(ctx.user_id),
        action,
        &body,
        recipient,
        category,
    )
    .await
    .map_err(database_error)?;
    crate::audit::record_in_transaction(
        &mut tx,
        Some(project),
        ctx.user_id,
        &format!("task.{action}"),
        "task",
        &id.to_string(),
        json!({}),
    )
    .await
    .map_err(database_error)?;
    let result: Value = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT {FIELDS} FROM tasks t WHERE id=$1"
    )))
    .bind(id)
    .fetch_one(&mut *tx)
    .await
    .map_err(database_error)?;
    sqlx::query("INSERT INTO task_commands(project_id,actor_user_id,command_id,digest,response) VALUES($1,$2,$3,$4,$5)").bind(project).bind(ctx.user_id).bind(key).bind(digest).bind(&result).execute(&mut *tx).await.map_err(database_error)?;
    tx.commit().await.map_err(database_error)?;
    Ok(Json(result))
}

async fn apply(
    tx: &mut Transaction<'_, Postgres>,
    project: Uuid,
    actor: Uuid,
    cmd: &mut Command,
    current: Option<&crate::team::TaskSnapshot>,
    id: Uuid,
    issue: Option<Uuid>,
) -> Result<(&'static str, String, Option<Uuid>, &'static str), Error> {
    Ok(match cmd {
        Command::Create { fields, .. } | Command::Edit { fields, .. } => {
            fields.validate()?;
            member(tx, project, fields.assignee).await?;
            if current.is_none() {
                if let Some(issue) = issue {
                    let exists: bool =
                        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tasks WHERE issue_id=$1)")
                            .bind(issue)
                            .fetch_one(&mut **tx)
                            .await
                            .map_err(database_error)?;
                    if exists {
                        return Err(error(StatusCode::CONFLICT, "issue_task_exists"));
                    }
                }
                sqlx::query("INSERT INTO tasks(id,project_id,issue_id,created_by,title,kind,priority) VALUES($1,$2,$3,$4,$5,$6,$7)").bind(id).bind(project).bind(issue).bind(actor).bind(&fields.title).bind(&fields.kind).bind(&fields.priority).execute(&mut **tx).await.map_err(database_error)?;
            }
            sqlx::query("UPDATE tasks SET title=$2,description=$3,kind=$4,priority=$5,assignee_user_id=$6,due_date=$7::text::date,reproduction=$8,root_cause=$9,version=version+$10,updated_at=now() WHERE id=$1")
                .bind(id).bind(&fields.title).bind(&fields.description).bind(&fields.kind).bind(&fields.priority).bind(fields.assignee).bind(&fields.due).bind(&fields.reproduction).bind(&fields.root_cause).bind(i64::from(current.is_some())).execute(&mut **tx).await.map_err(database_error)?;
            let changed = current
                .as_ref()
                .is_none_or(|r| r.assignee != fields.assignee);
            (
                if current.is_none() {
                    "created"
                } else {
                    "updated"
                },
                String::new(),
                fields.assignee,
                if changed { "assignment" } else { "task" },
            )
        }
        Command::Transition { stage, .. } => {
            let row = current
                .as_ref()
                .ok_or_else(|| error(StatusCode::NOT_FOUND, "not_found"))?;
            if !allowed(&row.stage, stage) {
                return Err(error(StatusCode::CONFLICT, "invalid_transition"));
            }
            sqlx::query("UPDATE tasks SET stage=$2,verified=false,fix_release='',automation=jsonb_build_object('regressionAt',clock_timestamp()),version=version+1,updated_at=now() WHERE id=$1").bind(id).bind(&*stage).execute(&mut **tx).await.map_err(database_error)?;
            if let Some(issue) = issue
                && (stage == "done" || stage == "inProgress")
            {
                sqlx::query("UPDATE issues SET status=$2,resolved_at=CASE WHEN $2='resolved' THEN now() ELSE NULL END,resolved_in_release=NULL,regressed_at=NULL,regressed_in_release=NULL WHERE id=$1").bind(issue).bind(if stage=="done"{"resolved"}else{"open"}).execute(&mut **tx).await.map_err(database_error)?;
                sqlx::query("INSERT INTO issue_activity(id,issue_id,kind,body,actor_user_id) VALUES($1,$2,'status',$3,$4)").bind(Uuid::now_v7()).bind(issue).bind(json!({"taskId":id,"stage":stage})).bind(actor).execute(&mut **tx).await.map_err(database_error)?;
            }
            ("stage", stage.clone(), row.assignee, "task")
        }
        Command::Comment { body, mention, .. } => {
            let body = body.trim();
            if body.is_empty() || body.len() > 16000 {
                return Err(error(StatusCode::BAD_REQUEST, "invalid_comment"));
            }
            member(tx, project, *mention).await?;
            sqlx::query("UPDATE tasks SET version=version+1,updated_at=now() WHERE id=$1")
                .bind(id)
                .execute(&mut **tx)
                .await
                .map_err(database_error)?;
            (
                "comment",
                body.to_owned(),
                mention.or_else(|| current.as_ref().and_then(|r| r.assignee)),
                if mention.is_some() { "mention" } else { "task" },
            )
        }
    })
}
