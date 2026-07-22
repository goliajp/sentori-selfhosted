//! GET /v1/_describe — lightweight self-describing endpoint
//! listing every route on the binary. Used by sentori-cli to
//! version-detect features + by SDK generators to bootstrap
//! TypeScript wrappers without manual surface duplication.

use axum::Json;
use serde_json::{Value, json};

// A single JSON literal describing the API surface; its length is the
// size of that literal, not of any logic.
#[allow(clippy::too_many_lines)]
pub async fn describe() -> Json<Value> {
    Json(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "sdk_token_prefix": "st_pk_",
        "session_cookie": "sentori_session",
        "endpoints": {
            "sdk_v1": [
                "POST /v1/events",
                "POST /v1/events:batch",
                "POST /v1/events/{event_id}/attachments/{kind}",
                "GET  /v1/events/_recent (SSE)",
                "POST /v1/heartbeat",
                "POST /v1/sessions",
                "POST /v1/deploys",
                "POST /v1/spans",
                "POST /v1/spans:batch",
                "POST /v1/metrics:batch",
                "POST /v1/runtime-metrics:batch",
                "POST /v1/track:batch",
                "POST /v1/security:report",
                "POST /v1/security/link",
                "GET  /v1/security/score",
                "GET  /v1/control/poll",
                "POST /v1/user-reports",
                "POST /v1/push/tokens",
                "DELETE /v1/push/tokens/{handle}",
                "POST /v1/push/tokens/{handle}/topics",
                "DELETE /v1/push/tokens/{handle}/topics/{topic}",
                "POST /v1/push/send",
                "GET  /v1/push/receipts/{send_id}",
                "POST /v1/push/sends/{send_id}/ack",
                "POST /v1/push/expo-compat/send",
                "GET  /v1/push/expo-compat/receipts/{send_id}",
                "GET  /v1/push/users/{fp_hex}/preferences",
                "PUT  /v1/push/users/{fp_hex}/preferences/{category}"
            ],
            "auth": [
                "POST /auth/register",
                "POST /auth/login",
                "POST /auth/verify",
                "POST /auth/forgot-password",
                "POST /auth/reset-password",
                "POST /auth/change-password",
                "GET  /auth/me",
                "POST /auth/logout",
                "GET  /auth/sessions",
                "DELETE /auth/sessions/{id_hash_hex}"
            ],
            "dashboard_v1": [
                "GET /v1/projects",
                "GET /v1/projects/{project_id}/issues",
                "GET /v1/projects/{project_id}/issues/{issue_id}",
                "PATCH /v1/projects/{project_id}/issues/{issue_id}",
                "POST /v1/projects/{project_id}/issues/_bulk_patch",
                "GET /v1/projects/{project_id}/events",
                "GET /v1/projects/{project_id}/events/trend",
                "GET /v1/projects/{project_id}/events/_recent (SSE)",
                "GET /v1/projects/{project_id}/traces",
                "GET /v1/projects/{project_id}/traces/{trace_id}",
                "GET /v1/projects/{project_id}/metrics",
                "GET /v1/projects/{project_id}/metrics/{name}/timeseries",
                "GET /v1/projects/{project_id}/replays",
                "GET /v1/projects/{project_id}/replays/{replay_id}/ndjson",
                "GET /v1/projects/{project_id}/stats",
                "GET /v1/projects/{project_id}/search",
                "GET /v1/projects/{project_id}/cert/observations",
                "GET /v1/projects/{project_id}/cert/watches",
                "GET /v1/projects/{project_id}/alerts",
                "GET /v1/usage",
                "GET /v1/audit",
                "GET /v1/alerts",
                "GET /v1/alerts/{id}",
                "PATCH /v1/alerts/{id}",
                "DELETE /v1/alerts/{id}",
                "GET /v1/saved-views",
                "GET /v1/saved-views/{id}",
                "PATCH /v1/saved-views/{id}",
                "DELETE /v1/saved-views/{id}",
                "POST /v1/saved-views",
                "GET /v1/issues/{issue_id}/watchers",
                "GET /v1/issues/{issue_id}/comments",
                "GET /v1/issues/{issue_id}/activity"
            ],
            "admin": [
                "GET POST DELETE /admin/api/projects/{project_id}/tokens",
                "DELETE /admin/api/tokens/{token_id}",
                "POST PATCH DELETE /admin/api/projects",
                "GET POST DELETE PATCH /admin/api/projects/{project_id}/push/credentials",
                "GET PATCH DELETE /admin/api/members",
                "GET POST DELETE /admin/api/invites",
                "POST DELETE /admin/api/projects/{project_id}/cert/watches",
                "GET POST DELETE PATCH /admin/api/projects/{project_id}/integrations",
                "GET POST DELETE /admin/api/projects/{project_id}/endpoint-probes",
                "PATCH DELETE /admin/api/endpoint-probes/{probe_id}",
                "POST DELETE /admin/api/issues/{issue_id}/watchers",
                "POST DELETE /admin/api/issues/{issue_id}/comments",
                "GET /admin/api/projects/{project_id}/releases",
                "GET /admin/api/projects/{project_id}/releases/{release_id}/artifacts",
                "DELETE /admin/api/releases/{release_id}",
                "GET /admin/api/saas/workspaces (saasadmin)",
                "GET /admin/api/saas/stats (saasadmin)"
            ]
        }
    }))
}
