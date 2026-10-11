//! Project-scoped reads and operator metadata for automatically discovered apps.

use std::sync::Arc;

use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use tracing::warn;
use uuid::Uuid;

use crate::{session_mw::SessionContext, state::AppState};

type Reply = (StatusCode, Json<Value>);

fn internal(error: &sqlx::Error) -> Reply {
    warn!(%error, "app query failed");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error":"internal"})),
    )
}

pub async fn summaries(pool: &PgPool, project_id: Uuid) -> Result<Vec<Value>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT jsonb_build_object(
           'id',a.id,'projectId',a.project_id,'platform',a.platform,'identifier',a.identifier,
           'name',a.name,'repositoryUrl',a.repository_url,'version',a.version,
           'createdAt',a.created_at,'lastEventAt',a.last_event_at,'lastSessionAt',a.last_session_at,
           'events24h',(SELECT count(*) FROM events e WHERE e.project_id=$1 AND e.app_id=a.id AND e.received_at>now()-interval '24 hours'),
           'errors24h',(SELECT count(*) FROM events e WHERE e.project_id=$1 AND e.app_id=a.id AND e.kind='error' AND e.received_at>now()-interval '24 hours'),
           'releaseCount',(SELECT count(*) FROM app_releases ar WHERE ar.app_id=a.id),
           'artifactSummary',coalesce((SELECT jsonb_agg(summary) FROM (
             SELECT f.kind,count(*) AS count,bool_or(f.usable IS TRUE) AS usable,
               bool_or(f.usable IS FALSE) AS broken,bool_or(f.usable IS NULL) AS unchecked
             FROM app_releases ar JOIN release_artifacts f ON f.release_id=ar.release_id
             WHERE ar.app_id=a.id AND (f.kind IN ('sourcemap','srcbundle')
               OR f.kind='dsym' AND a.platform='ios' OR f.kind='proguard' AND a.platform='android')
             GROUP BY f.kind) summary),'[]'::jsonb),
           'latestRelease',(SELECT jsonb_build_object('id',r.id,'name',r.name,'createdAt',r.created_at)
             FROM app_releases ar JOIN releases r ON r.id=ar.release_id WHERE ar.app_id=a.id
             ORDER BY r.created_at DESC,r.id DESC LIMIT 1)
         ) FROM apps a WHERE a.project_id=$1 ORDER BY a.platform COLLATE \"C\",a.identifier COLLATE \"C\",a.id",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
}

pub async fn project_summary(
    pool: &PgPool,
    project_id: Uuid,
) -> Result<(Vec<Value>, Vec<Value>), sqlx::Error> {
    let apps = summaries(pool, project_id).await?;
    let artifacts = sqlx::query_scalar(
        "SELECT jsonb_build_object('kind',a.kind,'usable',bool_or(a.usable IS TRUE),
           'broken',bool_or(a.usable IS FALSE),'unchecked',bool_or(a.usable IS NULL),'count',count(*))
         FROM release_artifacts a JOIN releases r ON r.id=a.release_id WHERE r.project_id=$1 GROUP BY a.kind",
    ).bind(project_id).fetch_all(pool).await?;
    Ok((apps, artifacts))
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Value>, Reply> {
    super::tokens::ensure_project_access(&state, &ctx, project_id).await?;
    Ok(Json(
        json!({"apps":summaries(&state.pool,project_id).await.map_err(|error| internal(&error))?}),
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateBody {
    expected_version: i64,
    name: Option<String>,
    repository_url: Option<String>,
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path((project_id, app_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateBody>,
) -> Result<Json<Value>, Reply> {
    if !ctx.role.is_superadmin() {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error":"superadmin_only"})),
        ));
    }
    super::tokens::ensure_project_access(&state, &ctx, project_id).await?;
    if body.expected_version < 1
        || body
            .name
            .as_ref()
            .is_some_and(|name| name.trim().chars().count() > 200)
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"invalid_app_name"})),
        ));
    }
    if let Some(value) = body
        .repository_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let valid = reqwest::Url::parse(value).is_ok_and(|url| {
            value.len() <= 2048
                && matches!(url.scheme(), "http" | "https")
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none()
                && url.query().is_none()
                && url.fragment().is_none()
        });
        if !valid {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error":"invalid_repository_url"})),
            ));
        }
    }
    let mut tx = state.pool.begin().await.map_err(|error| internal(&error))?;
    let version: Option<i64> = sqlx::query_scalar(
        "UPDATE apps SET name=CASE WHEN $4::text IS NULL THEN name ELSE nullif($4,'') END,
           repository_url=CASE WHEN $5::text IS NULL THEN repository_url ELSE nullif($5,'') END,
           version=version+1 WHERE id=$1 AND project_id=$2 AND version=$3 RETURNING version",
    )
    .bind(app_id)
    .bind(project_id)
    .bind(body.expected_version)
    .bind(body.name.as_deref().map(str::trim))
    .bind(body.repository_url.as_deref().map(str::trim))
    .fetch_optional(&mut *tx)
    .await
    .map_err(|error| internal(&error))?;
    let Some(version) = version else {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM apps WHERE id=$1 AND project_id=$2)")
                .bind(app_id)
                .bind(project_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(|error| internal(&error))?;
        return Err(if exists {
            (StatusCode::CONFLICT, Json(json!({"error":"app_changed"})))
        } else {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error":"app_not_found"})),
            )
        });
    };
    crate::audit::record_in_transaction(
        &mut tx,
        Some(project_id),
        ctx.user_id,
        "app.update",
        "app",
        &app_id.to_string(),
        json!({"name":body.name,"repositoryUrl":body.repository_url,"version":version}),
    )
    .await
    .map_err(|error| internal(&error))?;
    tx.commit().await.map_err(|error| internal(&error))?;
    Ok(Json(json!({"ok":true,"version":version})))
}
