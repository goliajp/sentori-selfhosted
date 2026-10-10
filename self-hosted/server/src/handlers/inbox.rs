//! Personal notifications, filtered against current authorization on every read/write.
use super::jobs::database_error;
use crate::{session_mw::SessionContext, state::AppState};
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;
type Error = (StatusCode, Json<Value>);
const ACCESS: &str = "i.user_id=$1 AND ($2 OR EXISTS(SELECT 1 FROM project_assignments a WHERE a.user_id=$1 AND a.project_id=i.project_id))";
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct List {
    project_id: Option<Uuid>,
    unread: Option<bool>,
    archived: Option<bool>,
    offset: Option<i64>,
    limit: Option<i64>,
}
pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Query(q): Query<List>,
) -> Result<Json<Value>, Error> {
    let offset = q.offset.unwrap_or(0).clamp(0, 1_000_000);
    let limit = q.limit.unwrap_or(50).clamp(1, 100);
    let query = format!(
        "WITH visible AS MATERIALIZED (SELECT i.* FROM inbox_items i WHERE {ACCESS}), selected AS MATERIALIZED (SELECT * FROM visible WHERE ($3::uuid IS NULL OR project_id=$3) AND (NOT $4 OR read_at IS NULL) AND (archived_at IS NOT NULL)=$5), page AS (SELECT jsonb_build_object('id',id,'projectId',project_id,'taskId',task_id,'monitorId',monitor_id,'category',category,'title',title,'body',body,'at',created_at,'read',read_at IS NOT NULL,'archived',archived_at IS NOT NULL) AS item FROM selected ORDER BY created_at DESC,id DESC LIMIT $6 OFFSET $7) SELECT (SELECT count(*) FROM selected) AS total,(SELECT count(*) FROM visible WHERE read_at IS NULL AND archived_at IS NULL) AS unread,COALESCE((SELECT jsonb_agg(item) FROM page),'[]'::jsonb) AS items"
    );
    let row = sqlx::query(sqlx::AssertSqlSafe(query))
        .bind(ctx.user_id)
        .bind(ctx.role.is_superadmin())
        .bind(q.project_id)
        .bind(q.unread.unwrap_or(false))
        .bind(q.archived.unwrap_or(false))
        .bind(limit)
        .bind(offset)
        .fetch_one(&state.pool)
        .await
        .map_err(database_error)?;
    let total: i64 = row.get("total");
    Ok(Json(
        json!({"items":row.get::<Value,_>("items"),"total":total,"unread":row.get::<i64,_>("unread"),"nextOffset":(offset+limit<total).then_some(offset+limit)}),
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Update {
    read: Option<bool>,
    archived: Option<bool>,
}
pub async fn update(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(id): Path<Uuid>,
    Json(body): Json<Update>,
) -> Result<Json<Value>, Error> {
    let query = format!(
        "UPDATE inbox_items i SET read_at=CASE WHEN $4::boolean IS NULL THEN read_at WHEN $4 THEN COALESCE(read_at,now()) ELSE NULL END,archived_at=CASE WHEN $5::boolean IS NULL THEN archived_at WHEN $5 THEN COALESCE(archived_at,now()) ELSE NULL END WHERE {ACCESS} AND id=$3"
    );
    let result = sqlx::query(sqlx::AssertSqlSafe(query))
        .bind(ctx.user_id)
        .bind(ctx.role.is_superadmin())
        .bind(id)
        .bind(body.read)
        .bind(body.archived)
        .execute(&state.pool)
        .await
        .map_err(database_error)?;
    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, Json(json!({"error":"not_found"}))));
    }
    Ok(Json(json!({"ok":true})))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "Four independent user notification subscriptions, not a state machine"
)]
pub struct Preferences {
    assignments: bool,
    mentions: bool,
    task_updates: bool,
    monitors: bool,
}
pub async fn preferences(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
) -> Result<Json<Value>, Error> {
    let row:Option<Value>=sqlx::query_scalar("SELECT jsonb_build_object('assignments',assignments,'mentions',mentions,'taskUpdates',task_updates,'monitors',monitors) FROM inbox_preferences WHERE user_id=$1").bind(ctx.user_id).fetch_optional(&state.pool).await.map_err(database_error)?;
    Ok(Json(row.unwrap_or_else(
        || json!({"assignments":true,"mentions":true,"taskUpdates":true,"monitors":true}),
    )))
}
pub async fn save_preferences(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Json(body): Json<Preferences>,
) -> Result<Json<Value>, Error> {
    sqlx::query("INSERT INTO inbox_preferences(user_id,assignments,mentions,task_updates,monitors) VALUES($1,$2,$3,$4,$5) ON CONFLICT(user_id) DO UPDATE SET assignments=$2,mentions=$3,task_updates=$4,monitors=$5").bind(ctx.user_id).bind(body.assignments).bind(body.mentions).bind(body.task_updates).bind(body.monitors).execute(&state.pool).await.map_err(database_error)?;
    Ok(Json(json!({"ok":true})))
}

pub async fn read_all(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
) -> Result<Json<Value>, Error> {
    let query = format!(
        "UPDATE inbox_items i SET read_at=now() WHERE {ACCESS} AND read_at IS NULL AND archived_at IS NULL"
    );
    let result = sqlx::query(sqlx::AssertSqlSafe(query))
        .bind(ctx.user_id)
        .bind(ctx.role.is_superadmin())
        .execute(&state.pool)
        .await
        .map_err(database_error)?;
    Ok(Json(json!({"updated":result.rows_affected()})))
}
