//! `/api/*` — the AI closed loop (design.md §9).
//!
//! Bearer api-scope token; every route is bound to the token's
//! project. The loop an agent runs without any human or Jira:
//!
//! ```text
//! GET  /api/issues?status=open      → pick work
//! GET  /api/issues/{id}/bundle     → full evidence (markdown)
//! POST /api/issues/{id}/notes      → "fixed in abc123, probe planted"
//! POST /api/issues/{id}/resolve    → anchor on the fixing release
//! ```

use std::sync::Arc;

use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use sentori_ingest_token::{IngestContext, Scope};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use tracing::warn;
use uuid::Uuid;

use crate::state::AppState;

fn require_api_scope(ctx: &IngestContext) -> Result<(), (StatusCode, Json<Value>)> {
    if ctx.scope == Scope::Api {
        Ok(())
    } else {
        Err((
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": "api_token_required",
                "hint": "this endpoint needs an `api`-scope token; the one used is `ingest` and ships inside the app",
            })),
        ))
    }
}

/// Load an issue and require it to belong to the token's project.
async fn load_scoped(
    state: &Arc<AppState>,
    ctx: &IngestContext,
    issue_id: Uuid,
) -> Result<(), (StatusCode, Json<Value>)> {
    let row: Option<(Uuid,)> = sqlx::query_as("SELECT project_id FROM issues WHERE id = $1")
        .bind(issue_id)
        .fetch_optional(&state.pool)
        .await
        .unwrap_or(None);
    match row {
        Some((pid,)) if pid == ctx.project_id => Ok(()),
        Some(_) | None => Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "issue_not_found" })),
        )),
    }
}

#[derive(Deserialize)]
pub struct ListQuery {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
}

pub async fn list(
    Extension(ctx): Extension<IngestContext>,
    State(state): State<Arc<AppState>>,
    Query(q): Query<ListQuery>,
) -> (StatusCode, Json<Value>) {
    if let Err(e) = require_api_scope(&ctx) {
        return e;
    }
    let status = q.status.unwrap_or_else(|| "open".to_string());
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    let rows = sqlx::query(
        "SELECT id, kind, group_title, message_sample, status, event_count, users_count, \
                max_per_user, last_seen, last_release, regressed_at \
         FROM issues WHERE project_id = $1 AND status = $2 \
           AND ($3::text IS NULL OR kind = $3) \
         ORDER BY (regressed_at IS NOT NULL) DESC, users_count DESC, max_per_user DESC \
         LIMIT $4",
    )
    .bind(ctx.project_id)
    .bind(&status)
    .bind(q.kind.as_deref())
    .bind(limit)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();
    let out: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<Uuid, _>("id"),
                "kind": r.get::<String, _>("kind"),
                "title": r.get::<String, _>("group_title"),
                "messageSample": r.get::<String, _>("message_sample"),
                "status": r.get::<String, _>("status"),
                "eventCount": r.get::<i64, _>("event_count"),
                "usersCount": r.get::<i64, _>("users_count"),
                "maxPerUser": r.get::<i64, _>("max_per_user"),
                "lastSeen": crate::wire_time::rfc3339(r.get("last_seen")),
                "lastRelease": r.get::<String, _>("last_release"),
                "regressed": r.get::<Option<time::OffsetDateTime>, _>("regressed_at").is_some(),
            })
        })
        .collect();
    (StatusCode::OK, Json(json!({ "issues": out })))
}

pub async fn bundle(
    Extension(ctx): Extension<IngestContext>,
    State(state): State<Arc<AppState>>,
    Path(issue_id): Path<Uuid>,
    Query(q): Query<BundleQuery>,
) -> axum::response::Response {
    if let Err(e) = require_api_scope(&ctx) {
        return e.into_response();
    }
    if let Err(e) = load_scoped(&state, &ctx, issue_id).await {
        return e.into_response();
    }
    match crate::bundle::assemble(&state.pool, issue_id).await {
        Ok(b) => {
            if q.format.as_deref() == Some("json") {
                (StatusCode::OK, Json(b.json)).into_response()
            } else {
                (
                    StatusCode::OK,
                    [(
                        axum::http::header::CONTENT_TYPE,
                        "text/markdown; charset=utf-8",
                    )],
                    b.markdown,
                )
                    .into_response()
            }
        }
        Err(crate::bundle::BundleError::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "issue_not_found" })),
        )
            .into_response(),
        Err(e) => {
            tracing::warn!(error = %e, "bundle assembly failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
                .into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct BundleQuery {
    #[serde(default)]
    pub format: Option<String>,
}

#[derive(Deserialize)]
pub struct NoteBody {
    pub body: String,
}

pub async fn add_note(
    Extension(ctx): Extension<IngestContext>,
    State(state): State<Arc<AppState>>,
    Path(issue_id): Path<Uuid>,
    Json(body): Json<NoteBody>,
) -> (StatusCode, Json<Value>) {
    if let Err(e) = require_api_scope(&ctx) {
        return e;
    }
    if let Err(e) = load_scoped(&state, &ctx, issue_id).await {
        return e;
    }
    // NULL actor + agent marker: the activity stream distinguishes
    // automation from humans by the body, not a user row.
    super::issues::record_activity(
        &state,
        issue_id,
        None,
        "note",
        json!({ "text": body.body, "via": "api" }),
    )
    .await;
    (StatusCode::CREATED, Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveBody {
    #[serde(default)]
    pub release: Option<String>,
}

pub async fn resolve(
    Extension(ctx): Extension<IngestContext>,
    State(state): State<Arc<AppState>>,
    Path(issue_id): Path<Uuid>,
    Json(body): Json<ResolveBody>,
) -> (StatusCode, Json<Value>) {
    if let Err(e) = require_api_scope(&ctx) {
        return e;
    }
    if let Err(e) = load_scoped(&state, &ctx, issue_id).await {
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
        tracing::warn!(error = %e, "api resolve failed");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "internal" })),
        );
    }
    super::issues::record_activity(
        &state,
        issue_id,
        None,
        "status",
        json!({ "to": "resolved", "inRelease": body.release, "via": "api" }),
    )
    .await;
    (StatusCode::OK, Json(json!({ "ok": true })))
}

/// Matches `/v1/events:batch`'s ceiling in spirit: a scan of a large
/// codebase is still a bounded thing, and an unbounded one is a way to
/// hold a connection open indefinitely.
const MAX_PROBE_REFS: usize = 1000;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeSyncBody {
    pub release: String,
    pub refs: Vec<String>,
}

/// POST /api/probes:sync — the CLI's static-scan registration
/// (design.md §2): every `sentori.probe(ref)` found in the source at
/// release-upload time lands here, so a silent probe is visibly
/// alive — distinguishable from deleted code.
pub async fn probes_sync(
    Extension(ctx): Extension<IngestContext>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<ProbeSyncBody>,
) -> (StatusCode, Json<Value>) {
    if let Err(e) = require_api_scope(&ctx) {
        return e;
    }
    // A CLI scanning a large codebase sends what it finds, and there
    // was no ceiling at all — one request could carry a million refs
    // and a million round trips with it. `/v1/events:batch` caps at
    // 200 for the same reason; this is the same shape of endpoint.
    if body.refs.len() > MAX_PROBE_REFS {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(json!({
                "error": "too_many_refs",
                "detail": format!("at most {MAX_PROBE_REFS} refs per call"),
                "sent": body.refs.len(),
            })),
        );
    }

    // Deduplicate before the insert. Postgres refuses to let one
    // statement's ON CONFLICT DO UPDATE touch the same row twice, and
    // a static scan that finds the same `sentori.probe(ref)` in two
    // files sends it twice — which the old row-at-a-time loop
    // tolerated by accident and a single statement does not.
    let mut seen = std::collections::HashSet::new();
    let refs: Vec<String> = body
        .refs
        .iter()
        .filter(|r| seen.insert(r.as_str()))
        .cloned()
        .collect();
    let ids: Vec<Uuid> = refs.iter().map(|_| Uuid::now_v7()).collect();

    // One statement. The loop counted only successes and reported the
    // number, so a database error left the caller with
    // `registered: 3` for five refs and nothing to act on — not even
    // a log line.
    let inserted = sqlx::query(
        "INSERT INTO probes (id, project_id, ref, first_registered_release, last_seen_release) \
         SELECT i, $2, r, $3, $3 FROM unnest($1::uuid[], $4::text[]) AS t(i, r) \
         ON CONFLICT (project_id, ref) \
         DO UPDATE SET last_seen_release = EXCLUDED.last_seen_release",
    )
    .bind(&ids)
    .bind(ctx.project_id)
    .bind(&body.release)
    .bind(&refs)
    .execute(&state.pool)
    .await;

    if let Err(e) = inserted {
        warn!(project_id = %ctx.project_id, error = %e, "probes.sync failed");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "internal" })),
        );
    }

    (
        StatusCode::OK,
        Json(json!({
            "registered": refs.len(),
            "duplicatesIgnored": body.refs.len() - refs.len(),
            "release": body.release,
        })),
    )
}
