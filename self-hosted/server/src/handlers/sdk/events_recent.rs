//! GET `/v1/events/_recent` — live event tail Server-Sent Events
//! stream, scoped to the bearer token's project.
//!
//! Subscribers see only events for their own project_id. Slow
//! consumers are dropped (`broadcast::error::RecvError::Lagged`)
//! rather than blocking the publisher.

use std::convert::Infallible;
use std::sync::Arc;

use axum::{
    Extension,
    extract::State,
    response::sse::{Event as SseEvent, KeepAlive, Sse},
};
use futures::stream::{Stream, StreamExt};
use sentori_ingest_token::IngestContext;
use serde_json::json;
use tokio_stream::wrappers::BroadcastStream;

use crate::state::AppState;

pub async fn handle(
    Extension(ctx): Extension<IngestContext>,
    State(state): State<Arc<AppState>>,
) -> Result<
    Sse<impl Stream<Item = Result<SseEvent, Infallible>>>,
    (axum::http::StatusCode, axum::Json<serde_json::Value>),
> {
    // Nothing in any SDK subscribes to this. It is an operator feed,
    // and a public token would hand a project's live error rate and
    // release names to anyone holding the app.
    super::require_admin_token(&ctx)?;

    let project_id = ctx.project_id.into_uuid();
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
