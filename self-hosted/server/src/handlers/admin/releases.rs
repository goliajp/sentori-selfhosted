//! Release admin endpoints — sourcemap + dsym + proguard
//! upload metadata, deploy markers.
//!
//! - `GET    /admin/api/projects/:project_id/releases` — list
//! - `GET    /admin/api/projects/:project_id/releases/:release_id/artifacts`
//! - `DELETE /admin/api/releases/:release_id` — delete (cascades artifacts)
//!
//! Create is handled by the SDK `/v1/deploys` endpoint; upload
//! of artifacts (sourcemap / dsym / proguard) flows through
//! `release-artifact` crate's separate ingest API.

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

use crate::handlers::admin::tokens::ensure_project_access;
use crate::session_mw::SessionContext;
use crate::state::AppState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListQuery {
    pub id: Option<Uuid>,
    pub app_id: Option<Uuid>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub q: Option<String>,
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    ensure_project_access(&state, &ctx, project_id).await?;
    let limit = q.limit.unwrap_or(200).min(500);
    let offset = q.offset.unwrap_or(0);
    if limit < 0
        || offset < 0
        || q.q
            .as_ref()
            .is_some_and(|value| value.chars().count() > 200)
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"invalid_query"})),
        ));
    }

    // Which platforms actually reported in each release. A missing
    // symbolication artifact only matters for a platform that is
    // sending events: three lights that go red regardless turn the
    // real gap into noise — insight-mobile shipped a release with no
    // dSYM while 100% of its traffic was iOS, and the android light
    // beside it was just as red (2026-08-10).
    let rows = sqlx::query(
        "WITH filtered AS MATERIALIZED (
           SELECT * FROM releases WHERE project_id = $1 AND ($6::uuid IS NULL OR id=$6) AND ($2::text IS NULL OR strpos(lower(name),lower($2))>0)
             AND ($5::uuid IS NULL OR EXISTS(SELECT 1 FROM app_releases ar WHERE ar.release_id=releases.id AND ar.app_id=$5 AND ar.project_id=$1))
         ) SELECT page.*, totals.total,
           ARRAY(SELECT DISTINCT a.platform FROM app_releases ar JOIN apps a ON a.id=ar.app_id
             WHERE ar.project_id=$1 AND ar.release_id=page.id AND ($5::uuid IS NULL OR a.id=$5)
               AND ar.last_seen_at IS NOT NULL) AS platforms,
           COALESCE((SELECT jsonb_agg(jsonb_build_object('id',a.id,'name',a.name,'identifier',a.identifier,'platform',a.platform) ORDER BY a.platform COLLATE \"C\",a.identifier COLLATE \"C\",a.id)
             FROM app_releases ar JOIN apps a ON a.id=ar.app_id WHERE ar.release_id=page.id AND ar.project_id=$1),'[]'::jsonb) AS apps,
           COALESCE((SELECT jsonb_agg(summary ORDER BY kind) FROM (
             SELECT kind, count(*) AS count, bool_or(usable IS TRUE) AS usable,
               bool_or(usable IS FALSE) AS broken, bool_or(usable IS NULL) AS unchecked
             FROM release_artifacts WHERE release_id=page.id GROUP BY kind
           ) summary),'[]'::jsonb) AS artifact_summary
         FROM (SELECT count(*) AS total FROM filtered) totals
         LEFT JOIN LATERAL (SELECT * FROM filtered ORDER BY created_at DESC,id DESC LIMIT $3 OFFSET $4) page ON true
         ORDER BY page.created_at DESC,page.id DESC",
    ).bind(project_id).bind(q.q.as_deref()).bind(limit).bind(offset).bind(q.app_id).bind(q.id)
    .fetch_all(&state.pool)
    .await
    // Was `unwrap_or_default()`, which reports a broken query as a
    // project with no releases — the same silence `list_artifacts`
    // below already had to be talked out of.
    .map_err(|e| {
        warn!(error = %e, "admin.releases.list query failed");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "internal" })),
        )
    })?;

    let total = rows.first().map_or(0, |row| row.get::<i64, _>("total"));
    let out: Vec<Value> = rows
        .iter()
        .filter(|row|row.get::<Option<Uuid>,_>("id").is_some())
        .map(|r| {
            json!({
                "id": r.get::<Uuid, _>("id").to_string(),
                "name": r.get::<String, _>("name"),
                "created_at": crate::wire_time::rfc3339(r.get::<time::OffsetDateTime, _>("created_at")),
                // No `usable` here. It is a property of an artifact —
                // one release has several, each parsed or not — and
                // the summary below preserves its three states. Details
                // still read full artifact rows. Asking a `releases` row for
                // it selected nothing and read it anyway: every call
                // to this route panicked on `ColumnNotFound("usable")`
                // from server 2.15.0 to 2.21.0, so the releases page
                // got a dropped connection rather than a list.
                "platforms": r.get::<Vec<String>, _>("platforms"),
                "artifactSummary": r.get::<Value, _>("artifact_summary"),
                "apps": r.get::<Value, _>("apps"),
            })
        })
        .collect();
    let next = offset.saturating_add(i64::try_from(out.len()).unwrap_or(i64::MAX));
    Ok(Json(
        json!({ "releases": out, "total": total, "offset": offset, "limit": limit,
            "nextOffset": if limit>0 && next<total {Some(next)} else {None},
        }),
    ))
}

pub async fn list_artifacts(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path((project_id, release_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    ensure_release_access(&state, &ctx, project_id, release_id).await?;
    // been called that. The query failed on every call, and
    // `unwrap_or_default()` turned the failure into an empty list — so
    // the releases page reported "no symbol files" for artifacts that
    // were sitting in the table. Errors now surface as errors; an
    // empty list has to mean empty.
    let rows = sqlx::query(
        "SELECT id, kind, name, content_hash, size_bytes, created_at, usable \
         FROM release_artifacts WHERE release_id = $1 ORDER BY created_at DESC",
    )
    .bind(release_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
    })?;

    let out: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<Uuid, _>("id").to_string(),
                "kind": r.get::<String, _>("kind"),
                "name": r.get::<String, _>("name"),
                "content_hash": r.get::<String, _>("content_hash"),
                "size_bytes": r.try_get::<i64, _>("size_bytes").unwrap_or(0),
                "created_at": crate::wire_time::rfc3339(r.get::<time::OffsetDateTime, _>("created_at")),
                // Selected since 2.15.0 and dropped here, which is the
                // mirror of the bug in `list` above: that one read a
                // column it had not selected, this one selected a
                // column it did not read. Same commit, both halves
                // wrong.
                //
                // The cost was the whole feature. `Releases.tsx` tests
                // `a.usable === false` to draw the ✕ and to list the
                // unreadable artifacts; against an absent field that is
                // never true, so every artifact counted as coverage and
                // "unreadable" was permanently empty — which is exactly
                // the state the check was built to end.
                //
                // NULL means never checked. Not the same claim as
                // checked and fine, so it stays distinct from `false`.
                "usable": r.get::<Option<bool>, _>("usable"),
            })
        })
        .collect();
    Ok(Json(json!({ "artifacts": out })))
}

/// The route carries no project in its path, so the project has to be
/// read from the row before the caller can be judged against it —
/// without that, any signed-in account could delete any project's
/// release, and `release_artifacts` cascades with it, which is how a
/// release's stacks stop being symbolicable.
pub async fn delete(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(release_id): Path<Uuid>,
) -> (StatusCode, Json<Value>) {
    let owner: Option<(Uuid,)> =
        match sqlx::query_as("SELECT project_id FROM releases WHERE id = $1")
            .bind(release_id)
            .fetch_optional(&state.pool)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                warn!(error = %e, "admin.releases delete_lookup_failed");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": "delete_failed" })),
                );
            }
        };
    // A release nobody may see and a release that does not exist answer
    // the same way, so this cannot be used to probe for ids.
    let Some((project_id,)) = owner else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "release_not_found" })),
        );
    };
    if let Err((code, body)) = ensure_project_access(&state, &ctx, project_id).await {
        return (code, body);
    }

    match sqlx::query("DELETE FROM releases WHERE id = $1")
        .bind(release_id)
        .execute(&state.pool)
        .await
    {
        // 200 with a body rather than 204: the console's fetch wrapper
        // parses every response as JSON, so a bodyless success reads to
        // it as a failed delete. Matches admin::projects::delete.
        Ok(_) => (StatusCode::OK, Json(json!({ "ok": true }))),
        Err(e) => {
            warn!(error = %e, "admin.releases delete_failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "delete_failed" })),
            )
        }
    }
}

/// A project path is not proof that the release belongs to it.
pub async fn ensure_release_access(
    state: &Arc<AppState>,
    ctx: &SessionContext,
    project_id: Uuid,
    release_id: Uuid,
) -> Result<(), (StatusCode, Json<Value>)> {
    ensure_project_access(state, ctx, project_id).await?;
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM releases WHERE id=$1 AND project_id=$2)",
    )
    .bind(release_id)
    .bind(project_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|error| {
        warn!(%error, "release access failed");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":"internal"})),
        )
    })?;
    if !exists {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error":"release_not_found"})),
        ));
    }
    Ok(())
}
