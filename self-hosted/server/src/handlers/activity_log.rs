//! GET /v1/issues/:issue_id/activity — issue activity log.
//! Append-only stream surface for IssueDetail's timeline tab.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Extension, Path, State};
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

use crate::session_mw::SessionContext;
use crate::state::AppState;

pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(issue_id): Path<Uuid>,
) -> Result<Json<Value>, super::tenant::ApiErr> {
    super::tenant::guard_issue(&state, ctx.workspace_id, issue_id).await?;

    let rows = sqlx::query(
        "SELECT id, actor_user_id, kind, payload, created_at \
         FROM activity_log WHERE issue_id = $1 ORDER BY created_at DESC LIMIT 200",
    )
    .bind(issue_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();
    let out: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<Uuid, _>("id").to_string(),
                "actor_user_id": r.try_get::<Option<Uuid>, _>("actor_user_id").ok().flatten().map(|u| u.to_string()),
                "kind": r.get::<String, _>("kind"),
                "payload": r.try_get::<Value, _>("payload").unwrap_or(Value::Null),
                "created_at": crate::wire_time::rfc3339(r.get::<time::OffsetDateTime, _>("created_at")),
            })
        })
        .collect();
    Ok(Json(json!({ "activity": out })))
}
