//! /v1/projects/:project_id/cert-monitor — K10 CT log monitor.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use sentori_workspace_identity::ProjectId;
use serde::Serialize;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::state::AppState;

#[derive(Serialize)]
pub struct WatchRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub domain: String,
    #[serde(with = "time::serde::rfc3339")]
    pub added_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_polled_at: Option<OffsetDateTime>,
}

#[derive(Serialize)]
pub struct ObservationRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub domain: String,
    pub common_name: Option<String>,
    pub issuer_name: String,
    #[serde(with = "time::serde::rfc3339")]
    pub not_before: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub not_after: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
}

/// Positional shape of the `cert_observations` query: id,
/// project_id, domain, common_name, issuer_name, not_before,
/// not_after, observed_at.
type CertObservationRow = (
    Uuid,
    Uuid,
    String,
    Option<String>,
    String,
    OffsetDateTime,
    OffsetDateTime,
    OffsetDateTime,
);

pub async fn list_watches(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<crate::session_mw::SessionContext>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<WatchRow>>, (StatusCode, String)> {
    super::tenant::guard_project(&state, ctx.workspace_id, project_id).await?;
    // K10 CertMonitor not yet plumbed into AppState (it's a
    // standalone service that needs reqwest client init).
    // v0.1 skeleton returns empty list; full wiring is
    // K10.1 follow-up.
    Ok(Json(Vec::new()))
}

pub async fn list_observations(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<crate::session_mw::SessionContext>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<ObservationRow>>, (StatusCode, String)> {
    super::tenant::guard_project(&state, ctx.workspace_id, project_id).await?;

    // Direct SQL read — bypass K10 service since we're just
    // reading the persisted observations table.
    let _pid = ProjectId::from_uuid(project_id);
    let rows: Vec<CertObservationRow> = sqlx::query_as(
        "SELECT id, project_id, domain, common_name, issuer_name,
                not_before, not_after, observed_at
         FROM cert_observations
         WHERE project_id = $1 AND workspace_id = $2
         ORDER BY observed_at DESC LIMIT 200",
    )
    .bind(project_id)
    .bind(ctx.workspace_id.into_uuid())
    .fetch_all(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(
        rows.into_iter()
            .map(
                |(
                    id,
                    pid,
                    domain,
                    common_name,
                    issuer_name,
                    not_before,
                    not_after,
                    observed_at,
                )| ObservationRow {
                    id,
                    project_id: pid,
                    domain,
                    common_name,
                    issuer_name,
                    not_before,
                    not_after,
                    observed_at,
                },
            )
            .collect(),
    ))
}
