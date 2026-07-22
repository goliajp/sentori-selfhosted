//! GET /v1/projects — list the caller's workspace's projects.
//!
//! Was an unscoped `SELECT ... FROM projects`, so it returned every
//! tenant's projects to any caller and handed out the ids the rest
//! of the dashboard API addresses data by.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Extension, State};
use axum::http::StatusCode;
use serde::Serialize;
use uuid::Uuid;

use crate::state::AppState;

#[derive(Serialize)]
pub struct ProjectRow {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<crate::session_mw::SessionContext>,
) -> Result<Json<Vec<ProjectRow>>, (StatusCode, String)> {
    // Scoping by workspace alone was only half of it. The `user` role
    // is defined as seeing the projects it has been granted and no
    // others, `project_user_visibility` holds those grants, and the
    // identity crate has carried a store for them all along — nothing
    // in this server had ever asked. A `user` saw every project in the
    // workspace, and a project id is the key the rest of this API
    // addresses data by.
    let mut sql = String::from("SELECT id, slug, name FROM projects WHERE workspace_id = $1");
    let visible: Option<Vec<Uuid>> = if ctx.role.auto_sees_all_projects() {
        None
    } else {
        let ids = state
            .identity_for(ctx.workspace_id)
            .visibility()
            .list_for_user(ctx.user_id)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        sql.push_str(" AND id = ANY($2)");
        Some(
            ids.into_iter()
                .map(sentori_workspace_identity::ProjectId::into_uuid)
                .collect(),
        )
    };
    sql.push_str(" ORDER BY created_at ASC");

    let mut q =
        sqlx::query_as::<_, (Uuid, String, String)>(&sql).bind(ctx.workspace_id.into_uuid());
    if let Some(ids) = &visible {
        q = q.bind(ids);
    }
    let rows: Vec<(Uuid, String, String)> = q
        .fetch_all(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(
        rows.into_iter()
            .map(|(id, slug, name)| ProjectRow { id, slug, name })
            .collect(),
    ))
}
