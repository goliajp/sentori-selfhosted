//! GET /v1/projects/:project_id/events/_recent — dashboard SSE
//! live tail. Same broadcast bus as the SDK-side
//! /v1/events/_recent, but scoped to a path-param project_id so
//! it can ride dashboard cookie auth instead of Bearer.

use std::convert::Infallible;
use std::sync::Arc;

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::sse::{Event as SseEvent, KeepAlive, Sse},
};
use futures::stream::{Stream, StreamExt};
use serde_json::json;
use tokio_stream::wrappers::BroadcastStream;
use uuid::Uuid;

use crate::state::AppState;

pub async fn handle(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<crate::session_mw::SessionContext>,
    Path(project_id): Path<Uuid>,
) -> Result<Sse<impl Stream<Item = Result<SseEvent, Infallible>>>, (StatusCode, String)> {
    // `RecentEventTick` carries no workspace_id, so the per-tick
    // filter below can only match on project_id. This guard, run
    // once before the subscription is handed out, is therefore the
    // sole tenant boundary on this endpoint.
    super::tenant::guard_project(&state, ctx.workspace_id, project_id).await?;

    let rx = state.events_bus.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(move |result| {
        let pid = project_id;
        async move {
            match result {
                Ok(tick) if tick.project_id == pid => {
                    let payload = json!({
                        "event_id": tick.event_id.to_string(),
                        "issue_id": tick.issue_id.to_string(),
                        "kind": tick.kind,
                        "platform": tick.platform,
                        "release": tick.release,
                        "environment": tick.environment,
                        "timestamp": crate::wire_time::rfc3339(tick.timestamp),
                    });
                    Some(Ok::<_, Infallible>(
                        SseEvent::default()
                            .event("event")
                            .json_data(payload)
                            .unwrap_or_default(),
                    ))
                }
                _ => None,
            }
        }
    });
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}
