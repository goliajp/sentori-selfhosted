//! /v1/saved-views — K15 saved view CRUD.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use sentori_saved_view::{SavedViewDraft, Scope, Target};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::handlers::tenant::{guard_project, guard_saved_view};
use crate::session_mw::SessionContext;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct ListQuery {
    pub target: String, // "issues" / "events" / "spans" / "replays" / "metrics"
}

#[derive(Serialize)]
pub struct ViewRow {
    pub id: Uuid,
    pub project_id: Option<Uuid>,
    pub target: String,
    pub scope: String,
    pub name: String,
    pub payload: Value,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

pub async fn list_workspace(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Vec<ViewRow>>, (StatusCode, String)> {
    let target =
        Target::from_db_str(&q.target).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let views = state
        .saved_views
        .list_workspace(ctx.workspace_id, target)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(
        views
            .into_iter()
            .map(|v| ViewRow {
                id: v.id,
                project_id: v
                    .project_id
                    .map(sentori_workspace_identity::ProjectId::into_uuid),
                target: v.target.to_string(),
                scope: v.scope.to_string(),
                name: v.name,
                payload: v.payload,
                created_at: v.created_at,
            })
            .collect(),
    ))
}

#[derive(Deserialize)]
pub struct CreateBody {
    pub name: String,
    pub target: String,
    pub project_id: Option<Uuid>,
    #[serde(default)]
    pub payload: Option<Value>,
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Json(body): Json<CreateBody>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, String)> {
    let target =
        Target::from_db_str(&body.target).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let mut draft = SavedViewDraft::new(ctx.workspace_id, &body.name, target, Scope::Workspace);
    if let Some(pid) = body.project_id {
        guard_project(&state, ctx.workspace_id, pid).await?;
        draft = draft.for_project(sentori_workspace_identity::ProjectId::from_uuid(pid));
    }
    if let Some(p) = body.payload {
        draft = draft.with_payload(p);
    }
    let id = state
        .saved_views
        .create(draft)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    Ok((StatusCode::CREATED, Json(serde_json::json!({"id": id}))))
}

pub async fn get(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    guard_saved_view(&state, ctx.workspace_id, id).await?;
    let view = state
        .saved_views
        .find(id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "saved_view not found".to_string()))?;
    Ok(Json(serde_json::json!({
        "id": view.id.to_string(),
        "name": view.name,
        "project_id": view.project_id.map(|u| u.to_string()),
        "target": format!("{:?}", view.target),
        "scope": format!("{:?}", view.scope),
        "user_id": view.user_id.map(|u| u.to_string()),
        "payload": view.payload,
        "created_at": crate::wire_time::rfc3339(view.created_at),
        "updated_at": crate::wire_time::rfc3339(view.updated_at),
    })))
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchBody {
    pub name: Option<String>,
    pub payload: Option<serde_json::Value>,
}

pub async fn patch(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(id): Path<Uuid>,
    Json(body): Json<PatchBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    use sentori_saved_view::SavedViewPatch;
    guard_saved_view(&state, ctx.workspace_id, id).await?;
    let p = SavedViewPatch {
        name: body.name,
        payload: body.payload,
    };
    state
        .saved_views
        .update(id, p)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    Ok(StatusCode::OK)
}

pub async fn delete(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, String)> {
    guard_saved_view(&state, ctx.workspace_id, id).await?;
    state
        .saved_views
        .delete(id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}
