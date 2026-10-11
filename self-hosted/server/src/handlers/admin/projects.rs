//! Project CRUD (superadmin).

use std::sync::Arc;

use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
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

type Reply = (StatusCode, Json<Value>);
type ProjectDetailRow = (
    String,
    Option<String>,
    time::OffsetDateTime,
    Option<String>,
    i64,
    Option<Vec<u8>>,
);

fn failure(error: &sqlx::Error) -> Reply {
    warn!(%error, "project transaction failed");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": "internal"})),
    )
}

fn validate(name: Option<&str>, platform: Option<&str>) -> Result<(), Reply> {
    if name.is_some_and(|name| name.trim().is_empty() || name.trim().chars().count() > 200) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid_project_name"})),
        ));
    }
    if platform.is_some_and(|platform| platform.trim().chars().count() > 80) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid_platform"})),
        ));
    }
    Ok(())
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Json(body): Json<CreateBody>,
) -> Result<Reply, Reply> {
    superadmin_only(&ctx)?;
    validate(Some(&body.name), body.platform.as_deref())?;
    let id = Uuid::now_v7();
    let name = body.name.trim();
    let platform = body
        .platform
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let mut tx = state.pool.begin().await.map_err(|error| failure(&error))?;
    if state.cloud.is_some() {
        crate::cloud_quota::project(&mut tx)
            .await
            .map_err(|error| error.reply())?;
    }
    sqlx::query("INSERT INTO projects (id, name, platform) VALUES ($1, $2, $3)")
        .bind(id)
        .bind(name)
        .bind(platform)
        .execute(&mut *tx)
        .await
        .map_err(|error| failure(&error))?;
    crate::audit::record_in_transaction(
        &mut tx,
        Some(id),
        ctx.user_id,
        "project.create",
        "project",
        &id.to_string(),
        json!({"name": name}),
    )
    .await
    .map_err(|error| failure(&error))?;
    tx.commit().await.map_err(|error| failure(&error))?;
    Ok((
        StatusCode::CREATED,
        Json(json!({"id": id, "name": name, "platform": platform, "version": 1})),
    ))
}

pub async fn get(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
) -> (StatusCode, Json<Value>) {
    if let Err(e) = super::tokens::ensure_project_access(&state, &ctx, project_id).await {
        return e;
    }
    let row: Option<ProjectDetailRow> = sqlx::query_as(
        "SELECT name, platform, created_at, webhook_url, version, webhook_secret_blob FROM projects WHERE id = $1",
    )
    .bind(project_id)
    .fetch_optional(&state.pool)
    .await
    .unwrap_or(None);
    match row {
        Some((name, platform, created_at, webhook_url, version, blob)) => (
            StatusCode::OK,
            Json(json!({
                "id": project_id,
                "name": name,
                "platform": platform,
                "createdAt": crate::wire_time::rfc3339(created_at),
                "version": version,
                // Only to whoever can set it. This route is not
                // superadmin-only — an admin assigned to the project
                // reaches it — and the URL is a secret in the same way
                // an ingest token is: whoever holds it can post into
                // the team's channel. `update` is superadmin-only, so
                // read follows write.
                //
                // I wrote the opposite here first, in a comment
                // claiming the route was superadmin-only. It is not.
                "webhookConfigured": webhook_url.is_some() || blob.is_some(),
                "webhookUrl": if ctx.role.is_superadmin() {
                    blob.as_ref().and_then(|bytes|state.vault.as_ref().and_then(|vault|crate::integrations::open(vault,project_id,"legacy_webhook",bytes).ok().map(|s|s.endpoint))).or(webhook_url)
                } else { None },
            })),
        ),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "project_not_found" })),
        ),
    }
}

// camelCase on the wire, like every other v1 body. Without the
// rename this field would only accept `webhook_url`, which the
// console does not send and `check-wire-case` does not read (it
// checks response keys, not request ones).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateBody {
    pub expected_version: Option<i64>,
    pub name: Option<String>,
    pub platform: Option<String>,
    /// Where issue alerts also go. Empty string clears it — absent and
    /// empty mean different things here, and a UI that cannot clear a
    /// field is a UI that cannot turn the channel off.
    pub webhook_url: Option<String>,
}

async fn stale_or_missing(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, id: Uuid) -> Reply {
    match sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM projects WHERE id = $1)")
        .bind(id)
        .fetch_one(&mut **tx)
        .await
    {
        Ok(true) => (
            StatusCode::CONFLICT,
            Json(json!({"error": "project_changed"})),
        ),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "project_not_found"})),
        ),
        Err(error) => failure(&error),
    }
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
    Json(body): Json<UpdateBody>,
) -> Result<Reply, Reply> {
    superadmin_only(&ctx)?;
    validate(body.name.as_deref(), body.platform.as_deref())?;
    let webhook = body.webhook_url.as_deref().map(|value| {
        let value = value.trim();
        if value.is_empty() { None } else { Some(value) }
    });
    let sealed = if let Some(Some(endpoint)) = webhook {
        crate::public_http::parse(endpoint)
            .map_err(|code| (StatusCode::BAD_REQUEST, Json(json!({"error":code}))))?;
        let vault = state.vault.as_ref().ok_or_else(|| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error":"vault_unconfigured"})),
            )
        })?;
        Some(
            crate::integrations::seal(
                vault,
                project_id,
                "legacy_webhook",
                &crate::integrations::Settings {
                    endpoint: endpoint.to_string(),
                    ..crate::integrations::Settings::default()
                },
            )
            .map_err(|code| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error":code})),
                )
            })?,
        )
    } else {
        None
    };
    let mut tx = state.pool.begin().await.map_err(|error| failure(&error))?;
    // Old admin clients may omit expectedVersion. New editors always send it.
    let version: Option<i64> = sqlx::query_scalar(
        "UPDATE projects SET name = COALESCE($2, name), \
         platform = CASE WHEN $3::text IS NULL THEN platform ELSE NULLIF($3, '') END, \
         webhook_url = CASE WHEN $5 THEN NULL ELSE webhook_url END, \
         webhook_secret_blob = CASE WHEN $5 THEN $4 ELSE webhook_secret_blob END, version = version + 1 \
         WHERE id = $1 AND ($6::bigint IS NULL OR version = $6) RETURNING version",
    )
    .bind(project_id)
    .bind(body.name.as_deref().map(str::trim))
    .bind(body.platform.as_deref().map(str::trim))
    .bind(sealed)
    .bind(webhook.is_some())
    .bind(body.expected_version)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|error| failure(&error))?;
    let Some(version) = version else {
        return Err(stale_or_missing(&mut tx, project_id).await);
    };
    crate::audit::record_in_transaction(&mut tx, Some(project_id), ctx.user_id, "project.update", "project", &project_id.to_string(),
        json!({"name": body.name.as_deref().map(str::trim), "platform": body.platform, "webhookUrl": webhook.map(|url| url.map_or("cleared", |_| "set")), "version": version}))
        .await.map_err(|error| failure(&error))?;
    tx.commit().await.map_err(|error| failure(&error))?;
    Ok((
        StatusCode::OK,
        Json(json!({"ok": true, "version": version})),
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteQuery {
    pub expected_version: Option<i64>,
}

pub async fn delete(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
    Query(query): Query<DeleteQuery>,
) -> Result<Reply, Reply> {
    superadmin_only(&ctx)?;
    let mut tx = state.pool.begin().await.map_err(|error| failure(&error))?;
    let result =
        sqlx::query("DELETE FROM projects WHERE id = $1 AND ($2::bigint IS NULL OR version = $2)")
            .bind(project_id)
            .bind(query.expected_version)
            .execute(&mut *tx)
            .await
            .map_err(|error| failure(&error))?;
    if result.rows_affected() == 0 {
        return Err(stale_or_missing(&mut tx, project_id).await);
    }
    crate::audit::record_in_transaction(
        &mut tx,
        None,
        ctx.user_id,
        "project.delete",
        "project",
        &project_id.to_string(),
        json!({}),
    )
    .await
    .map_err(|error| failure(&error))?;
    tx.commit().await.map_err(|error| failure(&error))?;
    Ok((StatusCode::OK, Json(json!({"ok": true}))))
}

/// The backend probe's story for one project: latest check + a
/// day's uptime. `Value::Null` when no URL is configured.
async fn backend_block(state: &Arc<AppState>, project_id: Uuid) -> Result<Value, sqlx::Error> {
    let url: Option<String> =
        sqlx::query_scalar("SELECT backend_health_url FROM projects WHERE id = $1")
            .bind(project_id)
            .fetch_one(&state.pool)
            .await?;
    let Some(url) = url else {
        return Ok(Value::Null);
    };
    let last = sqlx::query(
        "SELECT ok, status_code, latency_ms, checked_at FROM backend_checks \
         WHERE project_id = $1 ORDER BY checked_at DESC LIMIT 1",
    )
    .bind(project_id)
    .fetch_optional(&state.pool)
    .await?;
    let agg = sqlx::query(
        "SELECT count(*) AS total, count(*) FILTER (WHERE ok) AS ok_n \
         FROM backend_checks \
         WHERE project_id = $1 AND checked_at > now() - interval '24 hours'",
    )
    .bind(project_id)
    .fetch_one(&state.pool)
    .await?;
    let total = agg.get::<i64, _>("total");
    let ok_n = agg.get::<i64, _>("ok_n");
    Ok(json!({
        "url": url,
        "lastOk": last.as_ref().map(|r| sqlx::Row::get::<bool, _>(r, "ok")),
        "lastStatus": last.as_ref().and_then(|r| sqlx::Row::get::<Option<i32>, _>(r, "status_code")),
        "lastLatencyMs": last.as_ref().map(|r| sqlx::Row::get::<i32, _>(r, "latency_ms")),
        "lastCheckedAt": last.as_ref().map(|r| crate::wire_time::rfc3339(sqlx::Row::get(r, "checked_at"))),
        "checks24h": total,
        "ok24h": ok_n,
    }))
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
) -> Result<serde_json::Map<String, Value>, sqlx::Error> {
    let rows = sqlx::query(sql)
        .bind(project_id)
        .fetch_all(&state.pool)
        .await?;
    let mut out = serde_json::Map::new();
    for r in &rows {
        out.insert(
            sqlx::Row::get::<String, _>(r, "k"),
            Value::from(sqlx::Row::get::<i64, _>(r, "n")),
        );
    }
    Ok(out)
}

#[derive(Default, Deserialize)]
pub struct FilterOptionsQuery {
    #[serde(flatten)]
    pub filters: crate::event_filter::EventFilter,
    pub source: Option<String>,
    pub key: Option<String>,
}

/// Real event dimensions in the authorized project and selected app scope.
pub async fn environments(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
    Query(q): Query<FilterOptionsQuery>,
) -> (StatusCode, Json<Value>) {
    if let Err(error) = super::tokens::ensure_project_access(&state, &ctx, project_id).await {
        return error;
    }
    filter_options(&state, project_id, &q, "environments").await
}

pub async fn context_keys(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
    Query(q): Query<FilterOptionsQuery>,
) -> (StatusCode, Json<Value>) {
    if let Err(error) = super::tokens::ensure_project_access(&state, &ctx, project_id).await {
        return error;
    }
    filter_options(&state, project_id, &q, "keys").await
}

pub async fn context_values(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
    Query(q): Query<FilterOptionsQuery>,
) -> (StatusCode, Json<Value>) {
    if let Err(error) = super::tokens::ensure_project_access(&state, &ctx, project_id).await {
        return error;
    }
    filter_options(&state, project_id, &q, "values").await
}

async fn filter_options(
    state: &Arc<AppState>,
    project_id: Uuid,
    q: &FilterOptionsQuery,
    field: &str,
) -> (StatusCode, Json<Value>) {
    let filters = match q.filters.validated() {
        Ok(value) => value,
        Err(error) => return error,
    };
    let source = q.source.as_deref().unwrap_or("context");
    if !["context", "tags"].contains(&source)
        || (field == "values" && q.key.as_ref().is_none_or(|k| k.is_empty() || k.len() > 200))
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"invalid_filter", "field":"source_or_key"})),
        );
    }
    let (value, lateral) = match field {
        "keys" => (
            "dimension.key",
            "CROSS JOIN LATERAL jsonb_object_keys(CASE WHEN jsonb_typeof(e.payload->($3::text))='object' THEN e.payload->($3::text) ELSE '{}'::jsonb END) dimension(key)",
        ),
        "values" => ("e.payload->($3::text)->>($4::text)", ""),
        _ => ("e.environment", ""),
    };
    // Uniform parameter positions let the same event predicate serve all facets.
    let sql = format!(
        "SELECT {value} AS value FROM events e {lateral}
        WHERE e.project_id=$1 AND {} AND $3::text IS NOT NULL AND $4::text IS NOT NULL
          AND {value} IS NOT NULL
        GROUP BY value ORDER BY max(e.received_at) DESC, ({value}) COLLATE \"C\" LIMIT 200",
        crate::event_filter::predicate(2)
    );
    let result: Result<Vec<(String,)>, _> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(project_id)
        .bind(filters)
        .bind(source)
        .bind(q.key.as_deref().unwrap_or(""))
        .fetch_all(&state.pool)
        .await;
    match result {
        Ok(rows) => (
            StatusCode::OK,
            Json(json!({field: rows.into_iter().map(|(v,)| v).collect::<Vec<_>>()})),
        ),
        Err(error) => {
            warn!(%error, "event filter options failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error":"internal"})),
            )
        }
    }
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
) -> Result<Reply, Reply> {
    super::tokens::ensure_project_access(&state, &ctx, project_id).await?;
    let last_event_at: Option<time::OffsetDateTime> =
        sqlx::query_scalar("SELECT max(received_at) FROM events WHERE project_id = $1")
            .bind(project_id)
            .fetch_one(&state.pool)
            .await
            .map_err(|error| failure(&error))?;
    let mut counts = count_by(
        &state,
        project_id,
        "SELECT kind AS k, count(*) AS n FROM events \
         WHERE project_id = $1 AND received_at > now() - interval '24 hours' \
         GROUP BY kind",
    )
    .await
    .map_err(|error| failure(&error))?;
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
    .map_err(|error| failure(&error))?;
    let platforms = count_by(
        &state,
        project_id,
        "SELECT platform AS k, count(*) AS n FROM events \
         WHERE project_id = $1 AND received_at > now() - interval '24 hours' \
         GROUP BY platform",
    )
    .await
    .map_err(|error| failure(&error))?;
    let latest_release: Option<String> = sqlx::query_scalar(
        "SELECT release FROM events WHERE project_id = $1 ORDER BY received_at DESC,id DESC LIMIT 1",
    )
    .bind(project_id)
    .fetch_optional(&state.pool)
    .await.map_err(|error| failure(&error))?;
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
        .map_err(|error| failure(&error))?,
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
    .map_err(|error| failure(&error))?;
    let eligible = replay_row.get::<i64, _>("eligible");
    let with_screens = replay_row.get::<i64, _>("with_screens");
    let backend = backend_block(&state, project_id)
        .await
        .map_err(|error| failure(&error))?;
    let (apps, artifact_summary) = match super::apps::project_summary(&state.pool, project_id).await
    {
        Ok(summary) => summary,
        Err(error) => return Err(failure(&error)),
    };
    Ok((
        StatusCode::OK,
        Json(json!({
            "backend": backend,
            "apps": apps,
            "artifactSummary": artifact_summary,
            "lastEventAt": last_event_at.map(crate::wire_time::rfc3339),
            "counts24h": counts,
            "users24h": users_24h,
            "platforms24h": platforms,
            "latestRelease": latest_release,
            "latestReleaseArtifacts": artifact_kinds,
            "replay24h": { "eligible": eligible, "withScreens": with_screens },
        })),
    ))
}
