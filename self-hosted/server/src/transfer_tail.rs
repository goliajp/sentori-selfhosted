//! A source keeps its old SDK credentials locally and forwards only telemetry
//! using a newly issued target ingest token. The target proves its identity and
//! final archive before that token can be stored. Push and API routes stay local.
use crate::{public_http, state::AppState};
use axum::{
    Json,
    body::to_bytes,
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use sentori_ingest_token::{IngestContext, Scope};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

pub fn origin(value: &str) -> Result<String, &'static str> {
    let url = public_http::parse(value)?;
    if url.scheme() != "https" || url.path() != "/" || url.query().is_some() {
        return Err("https_origin_required");
    }
    Ok(url.origin().ascii_serialization())
}
pub fn token_headers(token: &str) -> Result<HeaderMap, &'static str> {
    if token.len() > 512 || !token.starts_with("st_") {
        return Err("invalid_ingest_token");
    }
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {token}")).map_err(|_| "invalid_ingest_token")?,
    );
    Ok(headers)
}
pub async fn check_target(
    origin: &str,
    token: &str,
    transfer: Uuid,
) -> Result<Value, &'static str> {
    let mut headers = token_headers(token)?;
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    let response = public_http::post(
        &format!("{origin}/v1/transfer-proof"),
        headers,
        serde_json::to_vec(&json!({"transferId":transfer})).map_err(|_| "invalid_transfer")?,
        true,
    )
    .await
    .map_err(|e| e.code)?;
    if response.status != 200 {
        return Err("target_not_active");
    }
    serde_json::from_slice(&response.body).map_err(|_| "invalid_target_proof")
}
pub(crate) fn unavailable() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        [(header::RETRY_AFTER, "10")],
        Json(json!({"error":"project_transfer_pending","retryAfterMs":10000})),
    )
        .into_response()
}
fn telemetry(path: &str) -> bool {
    matches!(path, "/v1/events" | "/v1/events:batch" | "/v1/deploys")
        || (path.starts_with("/v1/events/") && path.contains("/attachments/"))
}
#[derive(sqlx::FromRow)]
struct Route {
    transfer_id: Uuid,
    target_project_id: Uuid,
    target_origin: String,
    sealed_token: Vec<u8>,
}
// This single middleware owns the authenticated request and bounded response.
#[allow(clippy::too_many_lines)]
pub async fn admission(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    if request.method() != Method::POST || !telemetry(request.uri().path()) {
        return next.run(request).await;
    }
    let Some(ctx) = request.extensions().get::<IngestContext>().copied() else {
        return unavailable();
    };
    let fence: Result<Option<Uuid>, _> =
        sqlx::query_scalar("SELECT migration_fence FROM projects WHERE id=$1")
            .bind(ctx.project_id)
            .fetch_one(&state.pool)
            .await;
    let fence = match fence {
        Ok(None) => {
            let response = next.run(request).await;
            // A batch may straddle sealing between individual row commits.
            // Recheck after all handler writes: a 503 retries the whole batch,
            // whose event/session IDs deduplicate against the final snapshot.
            // Holding a pooled connection across next.run would starve ingest.
            let after: Result<Option<Uuid>, _> =
                sqlx::query_scalar("SELECT migration_fence FROM projects WHERE id=$1")
                    .bind(ctx.project_id)
                    .fetch_one(&state.pool)
                    .await;
            return if matches!(after, Ok(None)) {
                response
            } else {
                unavailable()
            };
        }
        Ok(Some(id)) => id,
        Err(_) => return unavailable(),
    };
    if ctx.scope != Scope::Ingest || request.headers().contains_key("sentori-transfer-hop") {
        return unavailable();
    }
    let route:Result<Option<Route>,_>=sqlx::query_as("SELECT r.transfer_id,r.target_project_id,r.target_origin,r.sealed_token FROM project_transfer_routes r JOIN project_transfers t ON t.id=r.transfer_id JOIN instance_settings i ON i.singleton AND i.id=t.instance_id AND i.generation=t.generation AND i.operating_mode='normal' WHERE r.project_id=$1 AND r.transfer_id=$2 AND r.enabled AND r.expires_at>clock_timestamp() AND t.direction='source' AND t.state='observing'")
        .bind(ctx.project_id).bind(fence).fetch_optional(&state.pool).await;
    let Ok(Some(route)) = route else {
        return unavailable();
    };
    let Some(vault) = &state.vault else {
        return unavailable();
    };
    let secret = vault
        .open(&route.sealed_token)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
    let Some(secret) = secret else {
        return unavailable();
    };
    if secret["transferId"] != route.transfer_id.to_string()
        || secret["projectId"] != ctx.project_id.to_string()
        || secret["targetProjectId"] != route.target_project_id.to_string()
        || secret["origin"] != route.target_origin
    {
        return unavailable();
    }
    let Some(token) = secret["token"].as_str() else {
        return unavailable();
    };
    let Ok(mut headers) = token_headers(token) else {
        return unavailable();
    };
    for name in [
        header::CONTENT_TYPE,
        header::CONTENT_ENCODING,
        axum::http::HeaderName::from_static("sentori-sdk"),
    ] {
        if let Some(value) = request.headers().get(&name) {
            headers.insert(name, value.clone());
        }
    }
    headers.insert("sentori-transfer-hop", HeaderValue::from_static("1"));
    let url = format!("{}{}", route.target_origin, request.uri().path());
    let limit = if request.uri().path().contains("/attachments/") {
        50 * 1024 * 1024
    } else {
        2 * 1024 * 1024
    };
    let Ok(Ok(body)) = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        to_bytes(request.into_body(), limit),
    )
    .await
    else {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(json!({"error":"invalid_transfer_body"})),
        )
            .into_response();
    };
    let result = public_http::post(&url, headers, body.to_vec(), true).await;
    let (success, error) = match &result {
        Ok(response) if (200..300).contains(&response.status) => (true, None),
        Ok(_) => (false, Some("target_rejected")),
        Err(error) => (false, Some(error.code)),
    };
    if let Err(error)=sqlx::query("UPDATE project_transfer_routes SET forwarded=forwarded+$3,failed=failed+$4,last_seen_at=clock_timestamp(),last_error=$5 WHERE project_id=$1 AND transfer_id=$2")
        .bind(ctx.project_id).bind(fence).bind(i64::from(success)).bind(i64::from(!success)).bind(error).execute(&state.pool).await {tracing::warn!(%error,"transfer tail accounting failed");}
    match result {
        Ok(response) if (200..300).contains(&response.status) => (
            StatusCode::from_u16(response.status).unwrap_or(StatusCode::BAD_GATEWAY),
            [
                (header::CONTENT_TYPE, "application/json"),
                (header::CACHE_CONTROL, "no-store"),
            ],
            response.body,
        )
            .into_response(),
        _ => unavailable(),
    }
}
