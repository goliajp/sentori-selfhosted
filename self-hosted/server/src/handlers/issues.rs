//! Issue surface — the product's main body (design.md §9/§11).
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
const STATUSES: [&str; 3] = ["open", "resolved", "ignored"];

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
    /// Deployment-environment filter: only issues with at least one
    /// event in this environment (issues aggregate across
    /// environments; the queue slices them).
    #[serde(default)]
    pub environment: Option<String>,
    /// Context-dimension filter (key + value together): only issues
    /// with at least one event whose payload.context[key] equals the
    /// value (text form). Sentori attaches no meaning to the key —
    /// the reader does.
    #[serde(default)]
    pub context_key: Option<String>,
    #[serde(default)]
    pub context_value: Option<String>,
    /// Release filter: issues with at least one event in this
    /// release (release stays out of the fingerprint — the
    /// resolve/regression narrative is cross-release).
    #[serde(default)]
    pub release: Option<String>,
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
        "lastSeen": crate::wire_time::rfc3339(r.get("last_seen")),
        "eventCount": r.get::<i64, _>("event_count"),
        "usersCount": r.get::<i64, _>("users_count"),
        "maxPerUser": r.get::<i64, _>("max_per_user"),
        "lastRelease": r.get::<String, _>("last_release"),
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
    // `clamp(1, 500)` turned `limit=0` into one row, so a caller
    // paging with a computed limit that reached zero got a phantom
    // record instead of an empty page. Zero means zero; a negative
    // limit is a caller bug and says so.
    let limit = match q.limit {
        Some(n) if n < 0 => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "invalid_limit", "detail": "limit must not be negative" })),
            );
        }
        Some(n) => n.min(500),
        None => 100,
    };
    // An unrecognised status returned an empty list at 200, and
    // `status=all` — the obvious guess for "show me everything" — was
    // indistinguishable from a project with nothing in it.
    let status = q.status.unwrap_or_else(|| "open".to_string());
    if !STATUSES.contains(&status.as_str()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "invalid_status",
                "detail": format!("status must be one of {}", STATUSES.join(", ")),
                "field": "status",
            })),
        );
    }
    if let Some(k) = q.kind.as_deref()
        && !["error", "warn", "trace", "assert", "probe"].contains(&k)
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "invalid_kind",
                "detail": "kind must be one of error, warn, trace, assert, probe",
                "field": "kind",
            })),
        );
    }
    let rows = sqlx::query(
        "SELECT * FROM issues \
         WHERE status = $1 \
           AND ($2::uuid IS NULL OR project_id = $2) \
           AND ($3::text IS NULL OR kind = $3) \
           AND ($4::uuid[] IS NULL OR project_id = ANY($4)) \
           AND ($6::text IS NULL \
                OR environment = $6 \
                OR (environment IS NULL AND EXISTS ( \
                      SELECT 1 FROM events e \
                      WHERE e.issue_id = issues.id AND e.environment = $6))) \
           AND ($7::text IS NULL OR $8::text IS NULL OR EXISTS ( \
                 SELECT 1 FROM events e \
                 WHERE e.issue_id = issues.id \
                   AND e.payload->'context'->>$7 = $8)) \
           AND ($9::text IS NULL OR EXISTS ( \
                 SELECT 1 FROM events e \
                 WHERE e.issue_id = issues.id AND e.release = $9)) \
         ORDER BY (regressed_at IS NOT NULL AND status = 'open') DESC, \
                  users_count DESC, max_per_user DESC, last_seen DESC \
         LIMIT $5",
    )
    .bind(&status)
    .bind(q.project_id)
    .bind(q.kind.as_deref())
    .bind(scope.as_deref())
    .bind(limit)
    .bind(q.environment.as_deref())
    .bind(q.context_key.as_deref())
    .bind(q.context_value.as_deref())
    .bind(q.release.as_deref())
    .fetch_all(&state.pool)
    .await;
    match rows {
        Ok(rows) => {
            let out: Vec<Value> = rows.iter().map(issue_row_json).collect();
            (StatusCode::OK, Json(json!({ "issues": out })))
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
    let row = sqlx::query("SELECT * FROM issues WHERE id = $1")
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
                 WHERE a.issue_id = $1 ORDER BY a.at DESC LIMIT 100",
            )
            .bind(issue_id)
            .fetch_all(&state.pool)
            .await
            .unwrap_or_default();
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
            .await
            .unwrap_or_default();
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
            let mut body = issue_row_json(&row);
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
    let r = sqlx::query(
        "UPDATE issues SET status = 'resolved', resolved_at = now(), \
         resolved_in_release = $2, regressed_at = NULL, regressed_in_release = NULL \
         WHERE id = $1",
    )
    .bind(issue_id)
    .bind(body.release.as_deref())
    .execute(&state.pool)
    .await;
    if let Err(e) = r {
        warn!(error = %e, "resolve failed");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "internal" })),
        );
    }
    record_activity(
        &state,
        issue_id,
        Some(ctx.user_id),
        "status",
        json!({ "to": "resolved", "inRelease": body.release, "note": body.note }),
    )
    .await;
    (StatusCode::OK, Json(json!({ "ok": true })))
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
    let r = sqlx::query("UPDATE issues SET status = $2 WHERE id = $1")
        .bind(issue_id)
        .bind(to)
        .execute(&state.pool)
        .await;
    if let Err(e) = r {
        warn!(error = %e, "status change failed");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "internal" })),
        );
    }
    record_activity(
        state,
        issue_id,
        Some(ctx.user_id),
        "status",
        json!({ "to": to }),
    )
    .await;
    (StatusCode::OK, Json(json!({ "ok": true })))
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
    let r = sqlx::query("UPDATE issues SET assignee_user_id = $2 WHERE id = $1")
        .bind(issue_id)
        .bind(body.user_id)
        .execute(&state.pool)
        .await;
    if let Err(e) = r {
        warn!(error = %e, "assign failed");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "internal" })),
        );
    }
    record_activity(
        &state,
        issue_id,
        Some(ctx.user_id),
        "assign",
        json!({ "assignee": body.user_id }),
    )
    .await;
    (StatusCode::OK, Json(json!({ "ok": true })))
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
    record_activity(
        &state,
        issue_id,
        Some(ctx.user_id),
        "note",
        json!({ "text": body.body }),
    )
    .await;
    (StatusCode::CREATED, Json(json!({ "ok": true })))
}

/// Occurrences of one issue — events only exist inside an issue
/// (webapp rule: no global event browser).
pub async fn occurrences(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(issue_id): Path<Uuid>,
) -> (StatusCode, Json<Value>) {
    if let Err(e) = load_issue(&state, &ctx, issue_id).await {
        return e;
    }
    // `screens_ref` rides each row so the dashboard can fall back to
    // the newest occurrence that HAS a visual replay — the newest
    // event alone may come from an older SDK that captured nothing,
    // and a replay from an hour ago beats no replay at all.
    let rows = sqlx::query(
        "SELECT e.id, e.kind, e.platform, e.occurred_at, e.received_at, \
                e.release, e.environment, e.user_key, sc.ref AS screens_ref \
         FROM events e \
         LEFT JOIN LATERAL ( \
             SELECT a.ref FROM event_attachments a \
             WHERE a.event_id = e.id AND a.project_id = e.project_id \
               AND a.kind = 'screens' LIMIT 1 \
         ) sc ON TRUE \
         WHERE e.issue_id = $1 ORDER BY e.received_at DESC LIMIT 100",
    )
    .bind(issue_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();
    let out: Vec<Value> = rows
        .iter()
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
    (StatusCode::OK, Json(json!({ "events": out })))
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
