//! Project-scoped push sends and actual provider attempts / SDK acknowledgements.
use super::push_commands::{self, Action, Command, Error, database_error, error};
use crate::{session_mw::SessionContext, state::AppState};
use axum::{
    Json,
    extract::{Extension, Path, Query, State},
    http::StatusCode,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ListQuery {
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub q: Option<String>,
    pub provider: Option<String>,
}
fn wire(row: &sqlx::postgres::PgRow) -> Value {
    let status: String = row.get("status");
    let outcome: Option<String> = row.get("provider_outcome");
    let display = if row.get::<Option<Uuid>, _>("lease_owner").is_some() {
        "sending"
    } else if status == "failed" && outcome.as_deref() == Some("uncertain") {
        "uncertain"
    } else if status == "failed" && outcome.as_deref() == Some("cancelled") {
        "cancelled"
    } else {
        &status
    };
    json!({"id":row.get::<Uuid,_>("id"),"token_id":row.get::<Uuid,_>("token_id"),"provider":row.get::<String,_>("provider"),"status":status,"displayState":display,"provider_outcome":outcome,"error":row.get::<Option<String>,_>("error"),"version":row.get::<i64,_>("version"),"retry_count":row.get::<i32,_>("retry_count"),"payload":row.get::<Value,_>("payload"),"credentialId":row.get::<Option<Uuid>,_>("credential_id"),"created_at":crate::wire_time::rfc3339(row.get("created_at")),"sent_at":crate::wire_time::rfc3339_opt(row.get("sent_at")),"next_attempt_at":crate::wire_time::rfc3339(row.get("next_attempt_at")),"deliveredAt":crate::wire_time::rfc3339_opt(row.get("acked_at"))})
}
pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project): Path<Uuid>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, Error> {
    super::tokens::ensure_project_access(&state, &ctx, project).await?;
    let limit = i64::from(query.limit.unwrap_or(50).clamp(1, 100));
    let offset = i64::from(query.offset.unwrap_or(0));
    let status = query.status.as_deref().filter(|s| !s.is_empty());
    let provider = query.provider.as_deref().filter(|s| !s.is_empty());
    let search = query.q.unwrap_or_default();
    if status.is_some_and(|s| {
        ![
            "queued",
            "sent",
            "failed",
            "uncertain",
            "sending",
            "cancelled",
        ]
        .contains(&s)
    }) || provider.is_some_and(|p| push_commands::config_keys(p).is_empty())
        || search.chars().count() > 200
    {
        return Err(error(StatusCode::BAD_REQUEST, "invalid_push_filter"));
    }
    let predicate = "project_id=$1 AND ($2::text IS NULL OR status=$2 OR ($2='sending' AND lease_owner IS NOT NULL) OR ($2 IN ('uncertain','cancelled') AND status='failed' AND provider_outcome=$2)) AND ($3::text IS NULL OR provider=$3) AND ($4='' OR position(lower($4) in lower(COALESCE(payload->>'title','')||' '||COALESCE(payload->>'body','')))>0)";
    let total: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT count(*) FROM push_sends WHERE {predicate}"
    )))
    .bind(project)
    .bind(status)
    .bind(provider)
    .bind(&search)
    .fetch_one(&state.pool)
    .await
    .map_err(database_error)?;
    let rows=sqlx::query(sqlx::AssertSqlSafe(format!("SELECT * FROM push_sends WHERE {predicate} ORDER BY created_at DESC,id DESC LIMIT $5 OFFSET $6"))).bind(project).bind(status).bind(provider).bind(search).bind(limit).bind(offset).fetch_all(&state.pool).await.map_err(database_error)?;
    Ok(Json(
        json!({"sends":rows.iter().map(wire).collect::<Vec<_>>(),"total":total,"offset":offset}),
    ))
}
pub async fn detail(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path((project, id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>, Error> {
    super::tokens::ensure_project_access(&state, &ctx, project).await?;
    let row = sqlx::query("SELECT * FROM push_sends WHERE id=$1 AND project_id=$2")
        .bind(id)
        .bind(project)
        .fetch_optional(&state.pool)
        .await
        .map_err(database_error)?
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "send_not_found"))?;
    let attempts:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('id',id,'attempt',attempt,'outcome',outcome,'httpStatus',provider_status,'durationMs',duration_ms,'at',created_at) FROM push_delivery_logs WHERE send_id=$1 ORDER BY created_at DESC,id DESC LIMIT 50").bind(id).fetch_all(&state.pool).await.map_err(database_error)?;
    Ok(Json(json!({"send":wire(&row),"attempts":attempts})))
}
pub async fn retry(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path((project, id)): Path<(Uuid, Uuid)>,
) -> Result<(StatusCode, Json<Value>), Error> {
    let value = push_commands::execute(
        &state,
        &ctx,
        project,
        Command {
            command_id: Uuid::now_v7().to_string(),
            action: Action::RetrySend {
                id,
                expected_version: None,
                acknowledge_duplicate: false,
            },
        },
    )
    .await?;
    Ok((StatusCode::ACCEPTED, Json(value)))
}
pub async fn retry_all_failed(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project): Path<Uuid>,
) -> Result<Json<Value>, Error> {
    super::tokens::ensure_project_access(&state, &ctx, project).await?;
    let ids:Vec<Uuid>=sqlx::query_scalar("SELECT p.id FROM push_sends p JOIN instance_settings s ON s.singleton AND s.generation=p.generation WHERE p.project_id=$1 AND p.status='failed' AND p.provider_outcome IS DISTINCT FROM 'uncertain' ORDER BY p.id LIMIT 1001").bind(project).fetch_all(&state.pool).await.map_err(database_error)?;
    if ids.is_empty() {
        return Ok(Json(json!({"requeued":0})));
    }
    push_commands::execute(
        &state,
        &ctx,
        project,
        Command {
            command_id: Uuid::now_v7().to_string(),
            action: Action::RetryFailed { ids },
        },
    )
    .await
    .map(Json)
}
/// `GET /admin/api/projects/:project_id/push/health`
///
/// One request behind the delivery card. The dashboard used to have
/// no way to see push at all; when it got one, aggregating a
/// thousand-row send list in the browser to answer "is delivery
/// working" would have been the wrong shape — the question is a
/// count, and counts belong in the database.
///
/// `reasons` is the point: "12 failed" is an alarm, "12 failed,
/// BadDeviceToken" is a fix.
pub async fn health(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    super::tokens::ensure_project_access(&state, &ctx, project_id).await?;

    let row = sqlx::query(
        "SELECT \
           count(*) FILTER (WHERE status = 'sent'   AND created_at > now() - interval '24 hours') AS sent24h, \
           count(*) FILTER (WHERE status = 'failed' AND created_at > now() - interval '24 hours') AS failed24h, \
           count(*) FILTER (WHERE status = 'queued') AS queued, \
           max(created_at) AS last_send_at \
         FROM push_sends WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| internal(&e))?;

    let reasons = sqlx::query(
        "SELECT coalesce(nullif(error, ''), provider_outcome, 'unknown') AS reason, count(*) AS n \
         FROM push_sends \
         WHERE project_id = $1 AND status = 'failed' \
           AND created_at > now() - interval '24 hours' \
         GROUP BY 1 ORDER BY n DESC LIMIT 8",
    )
    .bind(project_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| internal(&e))?;

    // `device_tokens` is the canonical store — send, subscribe and
    // preferences all query it, and it is the one carrying the
    // identity an issue can address.
    let tokens = sqlx::query(
        "SELECT count(*) FILTER (WHERE revoked_at IS NULL) AS live, \
                count(*) FILTER (WHERE revoked_at IS NOT NULL) AS quarantined, \
                count(*) FILTER (WHERE revoked_at IS NULL AND user_key IS NOT NULL) \
                  AS identified \
         FROM device_tokens WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| internal(&e))?;

    Ok(Json(json!({
        "sent24h": row.get::<i64, _>("sent24h"),
        "failed24h": row.get::<i64, _>("failed24h"),
        "queued": row.get::<i64, _>("queued"),
        "lastSendAt": crate::wire_time::rfc3339_opt(
            row.try_get::<Option<time::OffsetDateTime>, _>("last_send_at").ok().flatten()
        ),
        "liveTokens": tokens.get::<i64, _>("live"),
        "quarantinedTokens": tokens.get::<i64, _>("quarantined"),
        // Devices a specific user's issue can reach. The rest can
        // only be broadcast to.
        "identifiedTokens": tokens.get::<i64, _>("identified"),
        "reasons": reasons.iter().map(|r| json!({
            "reason": r.get::<String, _>("reason"),
            "count": r.get::<i64, _>("n"),
        })).collect::<Vec<_>>(),
    })))
}

fn internal(e: &sqlx::Error) -> (StatusCode, Json<Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": e.to_string() })),
    )
}
