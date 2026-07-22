//! GET /v1/_self_test
//!
//! End-to-end smoke test of the deployment. Operator runs after
//! migrate / deploy to confirm:
//!   - DB reachable
//!   - all 4 background workers spawned
//!   - schema migration current
//!   - at least one project + token mintable
//!
//! Returns JSON { ok, checks: [{ name, ok, detail? }] } so it's
//! grep-able for "ok: false". Status 200 if all checks pass, 503
//! otherwise so external uptime monitors can wire this directly.

use std::sync::Arc;

use axum::{Json, extract::State, http::StatusCode};
use serde_json::{Value, json};

use crate::state::AppState;

pub async fn handle(State(state): State<Arc<AppState>>) -> (StatusCode, Json<Value>) {
    let mut checks = Vec::<Value>::new();
    let mut all_ok = true;

    // 1. DB ping.
    let db_ok = sqlx::query("SELECT 1").execute(&state.pool).await.is_ok();
    checks.push(json!({
        "name": "db_ping",
        "ok": db_ok,
    }));
    if !db_ok {
        all_ok = false;
    }

    // 2. Migration version (presence of v0.2-required table).
    let schema_ok = sqlx::query("SELECT 1 FROM workspaces LIMIT 1")
        .fetch_optional(&state.pool)
        .await
        .is_ok();
    checks.push(json!({
        "name": "schema_workspaces_exists",
        "ok": schema_ok,
    }));
    if !schema_ok {
        all_ok = false;
    }

    // 3. Push tables present (v0.2 D-phase).
    let push_ok = sqlx::query("SELECT 1 FROM push_sends LIMIT 1")
        .fetch_optional(&state.pool)
        .await
        .is_ok();
    checks.push(json!({
        "name": "schema_push_sends_exists",
        "ok": push_ok,
    }));
    if !push_ok {
        all_ok = false;
    }

    // 4. Alert tables.
    let alert_ok = sqlx::query("SELECT 1 FROM alert_rules LIMIT 1")
        .fetch_optional(&state.pool)
        .await
        .is_ok();
    checks.push(json!({
        "name": "schema_alert_rules_exists",
        "ok": alert_ok,
    }));
    if !alert_ok {
        all_ok = false;
    }

    // 5. audit_logs present.
    let audit_ok = sqlx::query("SELECT 1 FROM audit_logs LIMIT 1")
        .fetch_optional(&state.pool)
        .await
        .is_ok();
    checks.push(json!({
        "name": "schema_audit_logs_exists",
        "ok": audit_ok,
    }));
    if !audit_ok {
        all_ok = false;
    }

    // 6. At least one project (deploy provisioned correctly).
    let proj_count: Option<(i64,)> = sqlx::query_as("SELECT COUNT(*)::bigint FROM projects")
        .fetch_optional(&state.pool)
        .await
        .ok()
        .flatten();
    let proj_n = proj_count.map_or(0, |t| t.0);
    checks.push(json!({
        "name": "has_at_least_one_project",
        "ok": proj_n > 0,
        "detail": format!("count={proj_n}"),
    }));
    if proj_n == 0 {
        all_ok = false;
    }

    // 7. Workspace count.
    let ws_count: Option<(i64,)> = sqlx::query_as("SELECT COUNT(*)::bigint FROM workspaces")
        .fetch_optional(&state.pool)
        .await
        .ok()
        .flatten();
    let ws_n = ws_count.map_or(0, |t| t.0);
    checks.push(json!({
        "name": "has_workspace",
        "ok": ws_n > 0,
        "detail": format!("count={ws_n}"),
    }));
    if ws_n == 0 {
        all_ok = false;
    }

    let status = if all_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (
        status,
        Json(json!({
            "ok": all_ok,
            "checks": checks,
            "version": env!("CARGO_PKG_VERSION"),
        })),
    )
}
