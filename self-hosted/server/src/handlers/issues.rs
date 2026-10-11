//! Issue surface — the product's main body.
//!
//! Serves both consumers of the issue system:
//! - the dashboard (cookie session, Inbox ordering = breadth×depth)
//! - the AI closed loop (Bearer api-scope token: list → bundle →
//!   note → resolve)
//!
//! Resolve anchors on a release; the regression side of that
//! contract lives in pipeline::is_regression.

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

/// The statuses an issue can be filtered by.
const STATUSES: [&str; 4] = ["open", "resolved", "ignored", "all"];

#[derive(Deserialize)]
pub struct ListQuery {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub project_id: Option<Uuid>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
    /// Literal, case-insensitive substring of title, sample or issue ID.
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub sort: Option<String>,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(flatten)]
    pub filters: crate::event_filter::EventFilter,
}

fn issue_row_json(r: &sqlx::postgres::PgRow) -> Value {
    json!({
        "id": r.get::<Uuid, _>("id"),
        "projectId": r.get::<Uuid, _>("project_id"),
        "kind": r.get::<String, _>("kind"),
        "title": r.get::<String, _>("group_title"),
        "messageSample": r.get::<String, _>("message_sample"),
        "surface": r.get::<Value, _>("surface"),
        "status": r.get::<String, _>("status"),
        "firstSeen": crate::wire_time::rfc3339(r.get("first_seen")),
        // `last_seen` is the device's clock — what the SDK said the
        // event happened at. `lastReceivedAt` is ours. A phone with a
        // wrong clock made the header read "2 months ago" while the
        // occurrence list below it read "2 minutes ago", both labelled
        // the same way, with nothing saying which was which.
        // `try_get` because the detail query does not compute it: a
        // caller that does not ask gets null rather than a panic.
        "lastSeen": crate::wire_time::rfc3339(r.try_get::<Option<time::OffsetDateTime>, _>("matched_last_seen").ok().flatten().unwrap_or_else(|| r.get("last_seen"))),
        "lastReceivedAt": r
            .try_get::<Option<time::OffsetDateTime>, _>("last_received_at")
            .ok()
            .flatten()
            .map(crate::wire_time::rfc3339),
        "eventCount": r.try_get::<Option<i64>, _>("matched_events").ok().flatten().unwrap_or_else(|| r.get("event_count")),
        "usersCount": r.try_get::<Option<i64>, _>("matched_users").ok().flatten().unwrap_or_else(|| r.get("users_count")),
        "maxPerUser": r.try_get::<Option<i64>, _>("matched_max_per_user").ok().flatten().unwrap_or_else(|| r.get("max_per_user")),
        "lastRelease": r.try_get::<Option<String>, _>("matched_last_release").ok().flatten().unwrap_or_else(|| r.get("last_release")),
        "assigneeUserId": r.get::<Option<Uuid>, _>("assignee_user_id"),
        "resolvedAt": crate::wire_time::rfc3339_opt(r.get("resolved_at")),
        "resolvedInRelease": r.get::<Option<String>, _>("resolved_in_release"),
        "regressedAt": crate::wire_time::rfc3339_opt(r.get("regressed_at")),
        "regressedInRelease": r.get::<Option<String>, _>("regressed_in_release"),
        // NULL on pre-split historical issues (aggregated across
        // environments/platforms before 2.9.0).
        "environment": r.get::<Option<String>, _>("environment"),
        "platform": r.get::<Option<String>, _>("platform"),
    })
}

/// Projects the caller may see: superadmin ⇒ no filter; admin ⇒
/// assignment rows.
async fn visible_projects(
    state: &Arc<AppState>,
    ctx: &SessionContext,
) -> Result<Option<Vec<Uuid>>, sqlx::Error> {
    if ctx.role.is_superadmin() {
        return Ok(None);
    }
    let rows: Vec<(Uuid,)> =
        sqlx::query_as("SELECT project_id FROM project_assignments WHERE user_id = $1")
            .bind(ctx.user_id)
            .fetch_all(&state.pool)
            .await?;
    Ok(Some(rows.into_iter().map(|(p,)| p).collect()))
}

struct ListWindow<'a> {
    limit: i64,
    offset: i64,
    status: &'a str,
    sort: &'a str,
    group: &'a str,
}

fn list_window(q: &ListQuery) -> Result<ListWindow<'_>, (StatusCode, Json<Value>)> {
    // `clamp(1, 500)` turned `limit=0` into one row, so a caller
    // paging with a computed limit that reached zero got a phantom
    // record instead of an empty page. Zero means zero; a negative
    // limit is a caller bug and says so.
    let limit = match q.limit {
        Some(n) if n < 0 => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "invalid_limit", "detail": "limit must not be negative" })),
            ));
        }
        Some(n) => n.min(500),
        None => 100,
    };
    let offset = q.offset.unwrap_or(0);
    let sort = q.sort.as_deref().unwrap_or("impact");
    let group = q.group.as_deref().unwrap_or("all");
    for (invalid, field) in [
        (offset < 0, "offset"),
        (!["impact", "newest"].contains(&sort), "sort"),
        (
            !["all", "open", "regressed", "fresh"].contains(&group),
            "group",
        ),
        (
            q.q.as_ref().is_some_and(|text| text.chars().count() > 200),
            "q",
        ),
    ] {
        if invalid {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": format!("invalid_{field}"), "field": field,
                })),
            ));
        }
    }
    // Keep the existing default for API clients; the dashboard can request all.
    let status = q.status.as_deref().unwrap_or("open");
    if !STATUSES.contains(&status) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "invalid_status",
                "detail": format!("status must be one of {}", STATUSES.join(", ")),
                "field": "status",
            })),
        ));
    }
    if let Some(k) = q.kind.as_deref()
        && !["error", "warn", "trace", "assert", "probe"].contains(&k)
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "invalid_kind",
                "detail": "kind must be one of error, warn, trace, assert, probe",
                "field": "kind",
            })),
        ));
    }
    Ok(ListWindow {
        limit,
        offset,
        status,
        sort,
        group,
    })
}

const ISSUE_LIST_SQL: &str = "WITH matching AS MATERIALIZED (
           SELECT e.id,e.issue_id,e.user_key,e.occurred_at,e.received_at,e.release,
                  count(*) OVER (PARTITION BY e.issue_id,e.user_key) AS per_user
           FROM events e WHERE $11 AND ($2::uuid IS NULL OR e.project_id=$2)
             AND ($4::uuid[] IS NULL OR e.project_id=ANY($4)) AND {event_predicate}
         ), event_stats AS (
           SELECT issue_id,count(*) AS matched_events,count(DISTINCT user_key) AS matched_users,
             coalesce(max(per_user) FILTER (WHERE user_key IS NOT NULL),0) AS matched_max_per_user,
             max(occurred_at) AS matched_last_seen,max(received_at) AS matched_last_received_at,
             (array_agg(release ORDER BY occurred_at DESC,id DESC))[1] AS matched_last_release
           FROM matching GROUP BY issue_id
         ), filtered AS MATERIALIZED (
           SELECT i.*,s.matched_events,s.matched_users,s.matched_max_per_user,s.matched_last_seen,
                  s.matched_last_received_at,s.matched_last_release
           FROM issues i LEFT JOIN event_stats s ON s.issue_id=i.id
           WHERE ($1 = 'all' OR i.status = $1)
             AND ($2::uuid IS NULL OR i.project_id = $2)
             AND ($3::text IS NULL OR i.kind = $3)
             AND ($4::uuid[] IS NULL OR i.project_id = ANY($4))
             AND (NOT $11 OR s.matched_events IS NOT NULL)
             AND ($7::text IS NULL OR strpos(lower(i.group_title), lower($7)) > 0
                  OR strpos(lower(i.message_sample), lower($7)) > 0 OR strpos(i.id::text, lower($7)) > 0)
             AND ($9 = 'all' OR ($9 = 'open' AND i.status = 'open')
                  OR ($9 = 'regressed' AND i.status = 'open' AND i.regressed_at IS NOT NULL)
                  OR ($9 = 'fresh' AND i.first_seen > now() - interval '24 hours'))
         )
         SELECT page.*, totals.total, totals.events, totals.open_count, totals.regressed_count,
                totals.release_count,
                coalesce(page.matched_last_received_at,(SELECT max(received_at) FROM events e WHERE e.issue_id = page.id)) AS last_received_at
         FROM (SELECT count(*) AS total, COALESCE(sum(coalesce(matched_events,event_count)), 0)::bigint AS events,
                      count(*) FILTER (WHERE status = 'open') AS open_count,
                      count(*) FILTER (WHERE status = 'open' AND regressed_at IS NOT NULL) AS regressed_count,
                      CASE WHEN $11 THEN (SELECT count(DISTINCT m.release) FROM matching m JOIN filtered f ON f.id=m.issue_id WHERE m.release<>'')
                           ELSE count(DISTINCT last_release) FILTER (WHERE last_release <> '') END AS release_count
               FROM filtered) totals
         LEFT JOIN LATERAL (
           SELECT * FROM filtered
           ORDER BY CASE WHEN $8 = 'impact' THEN (regressed_at IS NOT NULL AND status = 'open') END DESC,
                    CASE WHEN $8 = 'impact' THEN coalesce(matched_users,users_count) END DESC,
                    CASE WHEN $8 = 'impact' THEN coalesce(matched_max_per_user,max_per_user) END DESC,
                    coalesce(matched_last_seen,last_seen) DESC, id DESC
           LIMIT $5 OFFSET $10
         ) page ON true
         ORDER BY CASE WHEN $8 = 'impact' THEN (page.regressed_at IS NOT NULL AND page.status = 'open') END DESC,
                  CASE WHEN $8 = 'impact' THEN coalesce(page.matched_users,page.users_count) END DESC,
                  CASE WHEN $8 = 'impact' THEN coalesce(page.matched_max_per_user,page.max_per_user) END DESC,
                  coalesce(page.matched_last_seen,page.last_seen) DESC, page.id DESC";

/// Inbox list. Regressed floats above everything, then objective
/// importance (breadth desc, depth desc), then recency.
pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Query(q): Query<ListQuery>,
) -> (StatusCode, Json<Value>) {
    let scope = match visible_projects(&state, &ctx).await {
        Ok(s) => s,
        Err(e) => {
            warn!(error = %e, "visibility query failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            );
        }
    };
    let ListWindow {
        limit,
        offset,
        status,
        sort,
        group,
    } = match list_window(&q) {
        Ok(window) => window,
        Err(error) => return error,
    };
    // Materialize one authorized/filter snapshot for both the count and page.
    // The left join preserves totals for empty pages, including limit=0. Only
    // page rows compute the receipt clock. UUID is the stable ordering tie-break.
    let filters = match q.filters.validated() {
        Ok(filters) => filters,
        Err(error) => return error,
    };
    let filtered = filters["appId"] != Value::Null
        || filters["platform"] != Value::Null
        || filters["environment"] != Value::Null
        || filters["release"] != Value::Null
        || filters["tags"] != json!([]);
    let sql = ISSUE_LIST_SQL.replace("{event_predicate}", &crate::event_filter::predicate(6));
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(status)
        .bind(q.project_id)
        .bind(q.kind.as_deref())
        .bind(scope.as_deref())
        .bind(limit)
        .bind(&filters)
        .bind(
            q.q.as_deref()
                .map(str::trim)
                .filter(|text| !text.is_empty()),
        )
        .bind(sort)
        .bind(group)
        .bind(offset)
        .bind(filtered)
        .fetch_all(&state.pool)
        .await;
    match rows {
        Ok(rows) => {
            let total = rows.first().map_or(0, |row| row.get::<i64, _>("total"));
            let out: Vec<Value> = rows
                .iter()
                .filter(|row| row.get::<Option<Uuid>, _>("id").is_some())
                .map(issue_row_json)
                .collect();
            let next = offset.saturating_add(i64::try_from(out.len()).unwrap_or(i64::MAX));
            let summary = rows.first().map(|row| {
                json!({
                    "events": row.get::<i64, _>("events"),
                    "open": row.get::<i64, _>("open_count"),
                    "regressed": row.get::<i64, _>("regressed_count"),
                    "releases": row.get::<i64, _>("release_count"),
                })
            });
            (
                StatusCode::OK,
                Json(json!({ "issues": out, "total": total,
                    "limit": limit, "offset": offset, "summary": summary,
                    "nextOffset": if limit > 0 && next < total { Some(next) } else { None },
                })),
            )
        }
        Err(e) => {
            warn!(error = %e, "issue list failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}

async fn load_issue(
    state: &Arc<AppState>,
    ctx: &SessionContext,
    issue_id: Uuid,
) -> Result<sqlx::postgres::PgRow, (StatusCode, Json<Value>)> {
    // The same `last_received_at` the list computes. Without it here the
    // detail page — the one screen where the two clocks sit next to each
    // other — fell back to the device's and went on contradicting the
    // occurrence list underneath it.
    let row = sqlx::query(
        "SELECT *, \
                (SELECT max(received_at) FROM events e WHERE e.issue_id = issues.id) \
                  AS last_received_at \
         FROM issues WHERE id = $1",
    )
    .bind(issue_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        warn!(error = %e, "issue load failed");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "internal" })),
        )
    })?;
    let Some(row) = row else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "issue_not_found" })),
        ));
    };
    let project_id: Uuid = row.get("project_id");
    super::admin::tokens::ensure_project_access(state, ctx, project_id).await?;
    Ok(row)
}

pub async fn get(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(issue_id): Path<Uuid>,
) -> (StatusCode, Json<Value>) {
    match load_issue(&state, &ctx, issue_id).await {
        Ok(row) => {
            let activity = sqlx::query(
                "SELECT a.id, a.at, a.kind, a.body, a.actor_user_id, u.email AS actor_email \
                 FROM issue_activity a LEFT JOIN users u ON u.id = a.actor_user_id \
                 WHERE a.issue_id = $1 ORDER BY a.at DESC, a.id DESC LIMIT 100",
            )
            .bind(issue_id)
            .fetch_all(&state.pool)
            .await;
            let activity = match activity {
                Ok(rows) => rows,
                Err(error) => return internal(&error),
            };
            let acts: Vec<Value> = activity
                .iter()
                .map(|a| {
                    json!({
                        "id": a.get::<Uuid, _>("id"),
                        "at": crate::wire_time::rfc3339(a.get("at")),
                        "kind": a.get::<String, _>("kind"),
                        "body": a.get::<Value, _>("body"),
                        "actorUserId": a.get::<Option<Uuid>, _>("actor_user_id"),
                        "actorEmail": a.get::<Option<String>, _>("actor_email"),
                    })
                })
                .collect();
            // Release distribution — which versions this case has
            // appeared in, and at what volume. Release stays out of
            // the fingerprint (the resolve/regression narrative is
            // cross-release); this panel is how the version dimension
            // reads instead.
            let releases = sqlx::query(
                "SELECT release, count(*) AS events, \
                        min(occurred_at) AS first_at, max(occurred_at) AS last_at \
                 FROM events WHERE issue_id = $1 \
                 GROUP BY release ORDER BY max(occurred_at) DESC LIMIT 50",
            )
            .bind(issue_id)
            .fetch_all(&state.pool)
            .await;
            let releases = match releases {
                Ok(rows) => rows,
                Err(error) => return internal(&error),
            };
            let rels: Vec<Value> = releases
                .iter()
                .map(|r| {
                    json!({
                        "release": r.get::<String, _>("release"),
                        "events": r.get::<i64, _>("events"),
                        "firstAt": crate::wire_time::rfc3339(r.get("first_at")),
                        "lastAt": crate::wire_time::rfc3339(r.get("last_at")),
                    })
                })
                .collect();
            let assignees = sqlx::query(
                "SELECT id, email, display_name FROM users u WHERE role = 'superadmin'
                 OR EXISTS (SELECT 1 FROM project_assignments pa WHERE pa.user_id = u.id AND pa.project_id = $1)
                 ORDER BY lower(email), id",
            ).bind(row.get::<Uuid, _>("project_id")).fetch_all(&state.pool).await;
            let assignees = match assignees {
                Ok(rows) => rows,
                Err(error) => return internal(&error),
            };
            let mut body = issue_row_json(&row);
            body["assignees"] =
                json!(assignees.iter().map(|user| json!({
                "id": user.get::<Uuid, _>("id"), "email": user.get::<String, _>("email"),
                "displayName": user.get::<Option<String>, _>("display_name"),
            })).collect::<Vec<_>>());
            body["activity"] = Value::Array(acts);
            body["releases"] = Value::Array(rels);
            (StatusCode::OK, Json(body))
        }
        Err(e) => e,
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveBody {
    #[serde(default)]
    pub release: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

pub async fn resolve(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(issue_id): Path<Uuid>,
    Json(body): Json<ResolveBody>,
) -> (StatusCode, Json<Value>) {
    if let Err(e) = load_issue(&state, &ctx, issue_id).await {
        return e;
    }
    if body
        .release
        .as_ref()
        .is_some_and(|value| value.chars().count() > 200)
        || body
            .note
            .as_ref()
            .is_some_and(|value| value.chars().count() > 10000)
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid_resolution"})),
        );
    }
    let release = body
        .release
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    // One statement commits the decision and its evidence together. Retrying
    // an identical decision is a no-op; an explicit note is still a new fact.
    let result = sqlx::query(
        "WITH changed AS (
           UPDATE issues SET status = 'resolved', resolved_at = now(),
             resolved_in_release = $2, regressed_at = NULL, regressed_in_release = NULL
           WHERE id = $1 AND (status <> 'resolved' OR resolved_in_release IS DISTINCT FROM $2)
           RETURNING id
         ) INSERT INTO issue_activity (id, issue_id, actor_user_id, kind, body)
           SELECT $3, $1, $4, 'status', $5
           WHERE EXISTS (SELECT 1 FROM changed) OR $6::text IS NOT NULL",
    )
    .bind(issue_id)
    .bind(release)
    .bind(Uuid::now_v7())
    .bind(ctx.user_id)
    .bind(json!({"to": "resolved", "inRelease": release, "note": body.note}))
    .bind(body.note.as_deref())
    .execute(&state.pool)
    .await;
    match result {
        Ok(_) => (StatusCode::OK, Json(json!({"ok": true}))),
        Err(error) => internal(&error),
    }
}

pub async fn ignore(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(issue_id): Path<Uuid>,
) -> (StatusCode, Json<Value>) {
    set_status(&state, &ctx, issue_id, "ignored").await
}

pub async fn reopen(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(issue_id): Path<Uuid>,
) -> (StatusCode, Json<Value>) {
    set_status(&state, &ctx, issue_id, "open").await
}

async fn set_status(
    state: &Arc<AppState>,
    ctx: &SessionContext,
    issue_id: Uuid,
    to: &str,
) -> (StatusCode, Json<Value>) {
    if let Err(e) = load_issue(state, ctx, issue_id).await {
        return e;
    }
    let result = sqlx::query(
        "WITH changed AS (
           UPDATE issues SET status = $2 WHERE id = $1 AND status <> $2 RETURNING id
         ) INSERT INTO issue_activity (id, issue_id, actor_user_id, kind, body)
           SELECT $3, id, $4, 'status', $5 FROM changed",
    )
    .bind(issue_id)
    .bind(to)
    .bind(Uuid::now_v7())
    .bind(ctx.user_id)
    .bind(json!({"to": to}))
    .execute(&state.pool)
    .await;
    match result {
        Ok(_) => (StatusCode::OK, Json(json!({"ok": true}))),
        Err(error) => internal(&error),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignBody {
    pub user_id: Option<Uuid>,
}

pub async fn assign(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(issue_id): Path<Uuid>,
    Json(body): Json<AssignBody>,
) -> (StatusCode, Json<Value>) {
    if let Err(e) = load_issue(&state, &ctx, issue_id).await {
        return e;
    }
    let result = sqlx::query_scalar::<_, bool>(
        "WITH eligible AS (
           SELECT id FROM issues i WHERE id = $1 AND ($2::uuid IS NULL OR EXISTS (
             SELECT 1 FROM users u WHERE u.id = $2 AND (u.role = 'superadmin' OR EXISTS (
               SELECT 1 FROM project_assignments pa WHERE pa.user_id = u.id AND pa.project_id = i.project_id))))
         ), changed AS (
           UPDATE issues SET assignee_user_id = $2 WHERE id IN (SELECT id FROM eligible)
             AND assignee_user_id IS DISTINCT FROM $2 RETURNING id
         ), recorded AS (
           INSERT INTO issue_activity (id, issue_id, actor_user_id, kind, body)
           SELECT $3, id, $4, 'assign', $5 FROM changed
         ) SELECT EXISTS(SELECT 1 FROM eligible)",
    ).bind(issue_id).bind(body.user_id).bind(Uuid::now_v7()).bind(ctx.user_id)
     .bind(json!({"assignee": body.user_id})).fetch_one(&state.pool).await;
    match result {
        Ok(true) => (StatusCode::OK, Json(json!({"ok": true}))),
        Ok(false) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid_assignee"})),
        ),
        Err(error) => internal(&error),
    }
}

#[derive(Deserialize)]
pub struct NoteBody {
    pub body: String,
}

pub async fn add_note(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(issue_id): Path<Uuid>,
    Json(body): Json<NoteBody>,
) -> (StatusCode, Json<Value>) {
    if let Err(e) = load_issue(&state, &ctx, issue_id).await {
        return e;
    }
    if body.body.trim().is_empty() || body.body.chars().count() > 10000 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid_note"})),
        );
    }
    let result = sqlx::query(
        "INSERT INTO issue_activity (id, issue_id, actor_user_id, kind, body) VALUES ($1,$2,$3,'note',$4)",
    ).bind(Uuid::now_v7()).bind(issue_id).bind(ctx.user_id)
     .bind(json!({"text": body.body})).execute(&state.pool).await;
    match result {
        Ok(_) => (StatusCode::CREATED, Json(json!({"ok": true}))),
        Err(error) => internal(&error),
    }
}

fn internal(error: &sqlx::Error) -> (StatusCode, Json<Value>) {
    warn!(%error, "issue operation failed");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": "internal"})),
    )
}

#[derive(Deserialize)]
pub struct OccurrencesQuery {
    #[serde(flatten)]
    pub filters: crate::event_filter::EventFilter,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

/// Occurrences of one issue — events only exist inside an issue
/// (webapp rule: no global event browser).
pub async fn occurrences(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(issue_id): Path<Uuid>,
    Query(q): Query<OccurrencesQuery>,
) -> (StatusCode, Json<Value>) {
    if let Err(e) = load_issue(&state, &ctx, issue_id).await {
        return e;
    }
    let limit = q.limit.unwrap_or(100).min(500);
    let offset = q.offset.unwrap_or(0);
    if limit < 0 || offset < 0 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid_pagination"})),
        );
    }
    // `screens_ref` rides each row so the dashboard can fall back to
    // the newest occurrence that HAS a visual replay — the newest
    // event alone may come from an older SDK that captured nothing,
    // and a replay from an hour ago beats no replay at all.
    let filters = match q.filters.validated() {
        Ok(filters) => filters,
        Err(error) => return error,
    };
    let sql = format!("WITH filtered AS MATERIALIZED (
           SELECT e.* FROM events e WHERE e.issue_id=$1 AND {}
         ) SELECT page.*, totals.total, sc.ref AS screens_ref
         FROM (SELECT count(*) AS total FROM filtered) totals
         LEFT JOIN LATERAL (
           SELECT * FROM filtered ORDER BY received_at DESC, id DESC LIMIT $2 OFFSET $3
         ) page ON true
         LEFT JOIN LATERAL (
           SELECT a.ref FROM event_attachments a WHERE a.event_id = page.id AND a.project_id = page.project_id
             AND a.kind = 'screens' ORDER BY a.ref LIMIT 1
         ) sc ON true ORDER BY page.received_at DESC, page.id DESC", crate::event_filter::predicate(4));
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(issue_id)
        .bind(limit)
        .bind(offset)
        .bind(&filters)
        .fetch_all(&state.pool)
        .await;
    let rows = match rows {
        Ok(rows) => rows,
        Err(error) => return internal(&error),
    };
    let total = rows.first().map_or(0, |row| row.get::<i64, _>("total"));
    let out: Vec<Value> = rows
        .iter()
        .filter(|row| row.get::<Option<Uuid>, _>("id").is_some())
        .map(|r| {
            json!({
                "id": r.get::<Uuid, _>("id"),
                "kind": r.get::<String, _>("kind"),
                "platform": r.get::<String, _>("platform"),
                "occurredAt": crate::wire_time::rfc3339(r.get("occurred_at")),
                "receivedAt": crate::wire_time::rfc3339(r.get("received_at")),
                "release": r.get::<String, _>("release"),
                "environment": r.get::<String, _>("environment"),
                "userKey": r.get::<Option<String>, _>("user_key"),
                "screensRef": r.get::<Option<Uuid>, _>("screens_ref"),
            })
        })
        .collect();
    let next = offset.saturating_add(i64::try_from(out.len()).unwrap_or(i64::MAX));
    (
        StatusCode::OK,
        Json(
            json!({ "events": out, "total": total, "limit": limit, "offset": offset,
                "nextOffset": if limit > 0 && next < total { Some(next) } else { None },
            }),
        ),
    )
}

pub async fn record_activity(
    state: &Arc<AppState>,
    issue_id: Uuid,
    actor: Option<Uuid>,
    kind: &str,
    body: Value,
) {
    let r = sqlx::query(
        "INSERT INTO issue_activity (id, issue_id, actor_user_id, kind, body) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(Uuid::now_v7())
    .bind(issue_id)
    .bind(actor)
    .bind(kind)
    .bind(body)
    .execute(&state.pool)
    .await;
    if let Err(e) = r {
        warn!(error = %e, "activity write failed");
    }
}
