//! GET `/v1/security/score` — current trust score for caller's
//! (project, install_id/user_id) tuple.
//!
//! Phase C step 6 implementation: compute score = 100 minus the
//! count of security_events (kinds: pin_mismatch, root_detected,
//! debugger_attached, jailbreak, replay_attack) in the last 24h.
//! Floor at 0. This is the legacy v0.5 algorithm; refinements
//! (kind-weighted) land in a later phase.

use std::sync::Arc;

use axum::{
    Extension, Json,
    extract::{Query, State},
};
use sentori_ingest_token::IngestContext;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use tracing::info;

use crate::state::AppState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreQuery {
    #[serde(default)]
    pub install_id: Option<String>,
    #[serde(default)]
    pub user_id: Option<String>,
}

pub async fn handle(
    Extension(ctx): Extension<IngestContext>,
    Query(q): Query<ScoreQuery>,
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let mut sql = String::from(
        "SELECT COUNT(*) AS n FROM security_events \
         WHERE project_id = $1 \
           AND received_at > now() - interval '24 hours' \
           AND kind IN ('pin_mismatch','root_detected','debugger_attached','jailbreak','replay_attack')",
    );
    if q.install_id.is_some() {
        sql.push_str(" AND install_id = $2");
    } else if q.user_id.is_some() {
        sql.push_str(" AND user_id = $2");
    }

    let mut query = sqlx::query(&sql).bind(ctx.project_id.into_uuid());
    if let Some(ref iid) = q.install_id {
        query = query.bind(iid);
    } else if let Some(ref uid) = q.user_id {
        query = query.bind(uid);
    }

    let count: i64 = match query.fetch_one(&state.pool).await {
        Ok(row) => row.get::<i64, _>("n"),
        Err(_) => 0,
    };
    let score = (100i64 - count).max(0);

    info!(
        workspace_id = %ctx.workspace_id,
        project_id = %ctx.project_id,
        events_24h = count,
        score,
        "sdk.security_score",
    );
    Json(json!({ "score": score, "events_24h": count }))
}
