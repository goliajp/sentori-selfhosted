//! Project CRUD (superadmin) — design.md §9.

use std::sync::Arc;

use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tracing::warn;
use uuid::Uuid;

use crate::session_mw::SessionContext;
use crate::state::AppState;

fn superadmin_only(ctx: &SessionContext) -> Result<(), (StatusCode, Json<Value>)> {
    if ctx.role.is_superadmin() {
        Ok(())
    } else {
        Err((
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "superadmin_only" })),
        ))
    }
}

#[derive(Deserialize)]
pub struct CreateBody {
    pub name: String,
    #[serde(default)]
    pub platform: Option<String>,
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Json(body): Json<CreateBody>,
) -> (StatusCode, Json<Value>) {
    if let Err(e) = superadmin_only(&ctx) {
        return e;
    }
    let id = Uuid::now_v7();
    let platform = body.platform.unwrap_or_else(|| "react-native".to_string());
    match sqlx::query("INSERT INTO projects (id, name, platform) VALUES ($1, $2, $3)")
        .bind(id)
        .bind(&body.name)
        .bind(&platform)
        .execute(&state.pool)
        .await
    {
        Ok(_) => {
            crate::audit::record(
                &state.pool,
                Some(id),
                ctx.user_id,
                "project.create",
                "project",
                &id.to_string(),
                json!({ "name": body.name }),
            )
            .await;
            (
                StatusCode::CREATED,
                Json(json!({ "id": id, "name": body.name, "platform": platform })),
            )
        }
        Err(e) => {
            warn!(error = %e, "project create failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}

pub async fn get(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
) -> (StatusCode, Json<Value>) {
    if let Err(e) = super::tokens::ensure_project_access(&state, &ctx, project_id).await {
        return e;
    }
    let row: Option<(String, String, time::OffsetDateTime)> =
        sqlx::query_as("SELECT name, platform, created_at FROM projects WHERE id = $1")
            .bind(project_id)
            .fetch_optional(&state.pool)
            .await
            .unwrap_or(None);
    match row {
        Some((name, platform, created_at)) => (
            StatusCode::OK,
            Json(json!({
                "id": project_id,
                "name": name,
                "platform": platform,
                "createdAt": crate::wire_time::rfc3339(created_at),
            })),
        ),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "project_not_found" })),
        ),
    }
}

#[derive(Deserialize)]
pub struct UpdateBody {
    pub name: Option<String>,
    pub platform: Option<String>,
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
    Json(body): Json<UpdateBody>,
) -> (StatusCode, Json<Value>) {
    if let Err(e) = superadmin_only(&ctx) {
        return e;
    }
    let r = sqlx::query(
        "UPDATE projects SET name = COALESCE($2, name), platform = COALESCE($3, platform) \
         WHERE id = $1",
    )
    .bind(project_id)
    .bind(body.name.as_deref())
    .bind(body.platform.as_deref())
    .execute(&state.pool)
    .await;
    match r {
        Ok(res) if res.rows_affected() > 0 => {
            crate::audit::record(
                &state.pool,
                Some(project_id),
                ctx.user_id,
                "project.update",
                "project",
                &project_id.to_string(),
                json!({ "name": body.name, "platform": body.platform }),
            )
            .await;
            (StatusCode::OK, Json(json!({ "ok": true })))
        }
        Ok(_) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "project_not_found" })),
        ),
        Err(e) => {
            warn!(error = %e, "project update failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}

pub async fn delete(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
) -> (StatusCode, Json<Value>) {
    if let Err(e) = superadmin_only(&ctx) {
        return e;
    }
    let r = sqlx::query("DELETE FROM projects WHERE id = $1")
        .bind(project_id)
        .execute(&state.pool)
        .await;
    match r {
        Ok(res) if res.rows_affected() > 0 => {
            crate::audit::record(
                &state.pool,
                None,
                ctx.user_id,
                "project.delete",
                "project",
                &project_id.to_string(),
                json!({}),
            )
            .await;
            (StatusCode::OK, Json(json!({ "ok": true })))
        }
        Ok(_) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "project_not_found" })),
        ),
        Err(e) => {
            warn!(error = %e, "project delete failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}

/// The backend probe's story for one project: latest check + a
/// day's uptime. `Value::Null` when no URL is configured.
async fn backend_block(state: &Arc<AppState>, project_id: Uuid) -> Value {
    let url: Option<String> =
        sqlx::query_scalar("SELECT backend_health_url FROM projects WHERE id = $1")
            .bind(project_id)
            .fetch_one(&state.pool)
            .await
            .unwrap_or(None);
    let Some(url) = url else {
        return Value::Null;
    };
    let last = sqlx::query(
        "SELECT ok, status_code, latency_ms, checked_at FROM backend_checks \
         WHERE project_id = $1 ORDER BY checked_at DESC LIMIT 1",
    )
    .bind(project_id)
    .fetch_optional(&state.pool)
    .await
    .unwrap_or(None);
    let agg = sqlx::query(
        "SELECT count(*) AS total, count(*) FILTER (WHERE ok) AS ok_n \
         FROM backend_checks \
         WHERE project_id = $1 AND checked_at > now() - interval '24 hours'",
    )
    .bind(project_id)
    .fetch_one(&state.pool)
    .await
    .ok();
    let (total, ok_n) = agg.map_or((0i64, 0i64), |r| {
        (
            sqlx::Row::get::<i64, _>(&r, "total"),
            sqlx::Row::get::<i64, _>(&r, "ok_n"),
        )
    });
    json!({
        "url": url,
        "lastOk": last.as_ref().map(|r| sqlx::Row::get::<bool, _>(r, "ok")),
        "lastStatus": last.as_ref().and_then(|r| sqlx::Row::get::<Option<i32>, _>(r, "status_code")),
        "lastLatencyMs": last.as_ref().map(|r| sqlx::Row::get::<i32, _>(r, "latency_ms")),
        "lastCheckedAt": last.as_ref().map(|r| crate::wire_time::rfc3339(sqlx::Row::get(r, "checked_at"))),
        "checks24h": total,
        "ok24h": ok_n,
    })
}

/// One `GROUP BY` aggregate folded into a JSON map — the health
/// endpoint reads several of these.
async fn count_by(
    state: &Arc<AppState>,
    project_id: Uuid,
    // `'static`, so only a literal can reach it. sqlx 0.9 refuses a
    // runtime-built string here, and the honest answer is that this
    // helper never wanted one — every caller passes a literal.
    sql: &'static str,
) -> serde_json::Map<String, Value> {
    let rows = sqlx::query(sql)
        .bind(project_id)
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default();
    let mut out = serde_json::Map::new();
    for r in &rows {
        out.insert(
            sqlx::Row::get::<String, _>(r, "k"),
            Value::from(sqlx::Row::get::<i64, _>(r, "n")),
        );
    }
    out
}

/// GET /admin/api/projects/{id}/environments — the deployment
/// environments this project's events have actually reported, for
/// the queue's environment filter. Small by construction (a handful
/// of deployment targets), newest-traffic first.
pub async fn environments(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
) -> (StatusCode, Json<Value>) {
    if let Err(e) = super::tokens::ensure_project_access(&state, &ctx, project_id).await {
        return e;
    }
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT environment FROM events WHERE project_id = $1 \
         GROUP BY environment ORDER BY max(received_at) DESC",
    )
    .bind(project_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();
    let envs: Vec<String> = rows.into_iter().map(|(e,)| e).collect();
    (StatusCode::OK, Json(json!({ "environments": envs })))
}

/// GET /admin/api/projects/{id}/context-keys — the context keys this
/// project's events have actually reported. Sentori does not know
/// what `qa` or `tenant` MEAN — it only offers them as slicing
/// dimensions; the reader supplies the semantics (insight
/// context-dimensions feedback: generic capability for private
/// vocabulary, first-class fields only for universal concepts).
pub async fn context_keys(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
) -> (StatusCode, Json<Value>) {
    if let Err(e) = super::tokens::ensure_project_access(&state, &ctx, project_id).await {
        return e;
    }
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT key FROM events, \
                LATERAL jsonb_object_keys(payload->'context') AS key \
         WHERE project_id = $1 \
           AND jsonb_typeof(payload->'context') = 'object' \
         GROUP BY key ORDER BY max(received_at) DESC LIMIT 50",
    )
    .bind(project_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();
    let keys: Vec<String> = rows.into_iter().map(|(k,)| k).collect();
    (StatusCode::OK, Json(json!({ "keys": keys })))
}

#[derive(Deserialize)]
pub struct ContextValuesQuery {
    pub key: String,
}

/// GET /admin/api/projects/{id}/context-values?key=X — the values
/// that key has taken, newest-traffic first. Booleans and numbers
/// arrive as their text form ('true', '42') — the same form the
/// issues filter compares with.
pub async fn context_values(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
    Query(q): Query<ContextValuesQuery>,
) -> (StatusCode, Json<Value>) {
    if let Err(e) = super::tokens::ensure_project_access(&state, &ctx, project_id).await {
        return e;
    }
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT payload->'context'->>$2 AS val FROM events \
         WHERE project_id = $1 AND payload->'context' ? $2 \
           AND payload->'context'->>$2 IS NOT NULL \
         GROUP BY val ORDER BY max(received_at) DESC LIMIT 50",
    )
    .bind(project_id)
    .bind(&q.key)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();
    let values: Vec<String> = rows.into_iter().map(|(v,)| v).collect();
    (StatusCode::OK, Json(json!({ "values": values })))
}

/// GET /admin/api/projects/{id}/health — what the SDK's own traffic
/// says about the deployment, curated to the actionable set: is the
/// SDK alive (last event), how loud was the last day (per-kind
/// counts + distinct users), is one platform silently dark, what
/// release runs in the field and can its stacks be read (artifact
/// lights), and did error/warn events actually carry pixels
/// (replayScreens coverage).
pub async fn health(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
) -> (StatusCode, Json<Value>) {
    if let Err(e) = super::tokens::ensure_project_access(&state, &ctx, project_id).await {
        return e;
    }
    let last_event_at: Option<time::OffsetDateTime> =
        sqlx::query_scalar("SELECT max(received_at) FROM events WHERE project_id = $1")
            .bind(project_id)
            .fetch_one(&state.pool)
            .await
            .unwrap_or(None);
    let mut counts = count_by(
        &state,
        project_id,
        "SELECT kind AS k, count(*) AS n FROM events \
         WHERE project_id = $1 AND received_at > now() - interval '24 hours' \
         GROUP BY kind",
    )
    .await;
    // GROUP BY returns no row for a kind with no events, so a quiet
    // project answered `{}` — and the console, reading a missing key as
    // "unknown", drew a dash where the true answer is zero. It sat next
    // to users24h, which is a plain count and did say 0, so one card
    // showed two different renderings of the same fact.
    for kind in ["error", "warn", "trace", "assert", "probe"] {
        counts.entry(kind.to_string()).or_insert(json!(0));
    }
    let users_24h: i64 = sqlx::query_scalar(
        "SELECT count(DISTINCT user_key) FROM events \
         WHERE project_id = $1 AND received_at > now() - interval '24 hours' \
           AND user_key IS NOT NULL",
    )
    .bind(project_id)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);
    let platforms = count_by(
        &state,
        project_id,
        "SELECT platform AS k, count(*) AS n FROM events \
         WHERE project_id = $1 AND received_at > now() - interval '24 hours' \
         GROUP BY platform",
    )
    .await;
    let latest_release: Option<String> = sqlx::query_scalar(
        "SELECT release FROM events WHERE project_id = $1 ORDER BY received_at DESC LIMIT 1",
    )
    .bind(project_id)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(None);
    let artifact_kinds: Vec<String> = match &latest_release {
        Some(rel) => sqlx::query_scalar(
            "SELECT DISTINCT a.kind FROM release_artifacts a \
             JOIN releases r ON r.id = a.release_id \
             WHERE r.project_id = $1 AND r.name = $2",
        )
        .bind(project_id)
        .bind(rel)
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default(),
        None => Vec::new(),
    };
    let replay_row = sqlx::query(
        "SELECT count(*) AS eligible, \
                count(*) FILTER (WHERE EXISTS ( \
                    SELECT 1 FROM event_attachments a \
                    WHERE a.event_id = e.id AND a.project_id = e.project_id \
                      AND a.kind = 'screens')) AS with_screens \
         FROM events e \
         WHERE e.project_id = $1 AND e.received_at > now() - interval '24 hours' \
           AND e.kind IN ('error', 'warn')",
    )
    .bind(project_id)
    .fetch_one(&state.pool)
    .await
    .ok();
    let (eligible, with_screens) = replay_row.map_or((0, 0), |r| {
        (
            sqlx::Row::get::<i64, _>(&r, "eligible"),
            sqlx::Row::get::<i64, _>(&r, "with_screens"),
        )
    });
    let backend = backend_block(&state, project_id).await;
    (
        StatusCode::OK,
        Json(json!({
            "backend": backend,
            "lastEventAt": last_event_at.map(crate::wire_time::rfc3339),
            "counts24h": counts,
            "users24h": users_24h,
            "platforms24h": platforms,
            "latestRelease": latest_release,
            "latestReleaseArtifacts": artifact_kinds,
            "replay24h": { "eligible": eligible, "withScreens": with_screens },
        })),
    )
}
