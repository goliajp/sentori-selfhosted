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
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde_json::{Value, json};
use sqlx::Row;
use tracing::warn;
use uuid::Uuid;

use crate::state::AppState;

pub async fn list(
    State(state): State<Arc<AppState>>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    // Which platforms actually reported in each release. A missing
    // symbolication artifact only matters for a platform that is
    // sending events: three lights that go red regardless turn the
    // real gap into noise — insight-mobile shipped a release with no
    // dSYM while 100% of its traffic was iOS, and the android light
    // beside it was just as red (2026-08-10).
    let rows = sqlx::query(
        "SELECT r.id, r.name, r.created_at, \
                COALESCE(array_agg(DISTINCT e.platform) \
                         FILTER (WHERE e.platform IS NOT NULL), '{}') AS platforms \
         FROM releases r \
         LEFT JOIN events e ON e.project_id = r.project_id AND e.release = r.name \
         WHERE r.project_id = $1 \
         GROUP BY r.id, r.name, r.created_at \
         ORDER BY r.created_at DESC LIMIT 200",
    )
    .bind(project_id)
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

    let out: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<Uuid, _>("id").to_string(),
                "name": r.get::<String, _>("name"),
                "created_at": crate::wire_time::rfc3339(r.get::<time::OffsetDateTime, _>("created_at")),
                // No `usable` here. It is a property of an artifact —
                // one release has several, each parsed or not — and
                // `list_artifacts` returns it per row, which is what
                // the releases page reads. Asking a `releases` row for
                // it selected nothing and read it anyway: every call
                // to this route panicked on `ColumnNotFound("usable")`
                // from server 2.15.0 to 2.21.0, so the releases page
                // got a dropped connection rather than a list.
                "platforms": r.get::<Vec<String>, _>("platforms"),
            })
        })
        .collect();
    Ok(Json(json!({ "releases": out })))
}

pub async fn list_artifacts(
    State(state): State<Arc<AppState>>,
    Path((_project_id, release_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
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

pub async fn delete(
    State(state): State<Arc<AppState>>,
    Path(release_id): Path<Uuid>,
) -> StatusCode {
    match sqlx::query("DELETE FROM releases WHERE id = $1")
        .bind(release_id)
        .execute(&state.pool)
        .await
    {
        Ok(_) => StatusCode::NO_CONTENT,
        Err(e) => {
            warn!(error = %e, "admin.releases delete_failed");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}
