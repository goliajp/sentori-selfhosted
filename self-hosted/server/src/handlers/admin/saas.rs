//! Cross-workspace admin views — for SaaS-mode operators only.
//!
//! `sentori-server` serves both self-hosted (single workspace) and
//! SaaS (many workspaces in shared DB) deployments. These
//! endpoints read across all workspaces, intended for the
//! saasadmin webapp view.
//!
//! Self-hosted operators will see only their one workspace row
//! when calling these — that's fine, the row count is just 1.
//!
//! RBAC: gated by `session_middleware` plus `saasadmin_only`
//! (see `crate::saasadmin_mw`), which restricts the group to the
//! user ids in `SENTORI_SAASADMIN_USER_IDS`.
//!
//! Workspace create / delete / suspend / resume moved here from
//! the `sentori-saas-control` binary, which had its own account
//! system and no UI calling it. That binary is now only the
//! Stripe webhook receiver.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Extension, Path, State};
use axum::http::HeaderMap;
use axum::http::StatusCode;
use sentori_billing::{Plan, PlanStatus};
use sentori_workspace_identity::WorkspaceId;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;
use tracing::{info, warn};
use uuid::Uuid;

use crate::session_mw::SessionContext;
use crate::state::AppState;

/// Cross-tenant actions are the highest-consequence surface — an
/// operator touches a workspace that isn't theirs, and until this
/// existed there was no record beyond a tracing INFO line that gets
/// lost with rotation. Every mutating handler in this module now
/// calls it before it returns Ok.
///
/// The audit row goes into the *target* workspace's log rather than
/// the operator's. A tenant querying their own audit history sees who
/// suspended or reslotted them; without that the record is on the
/// wrong side of the transaction to be reachable.
///
/// `delete_workspace` audits *before* the DELETE, so the row exists at
/// insert time. `workspaces.id → audit_logs.workspace_id` is
/// `ON DELETE CASCADE`, so the row is destroyed with the workspace —
/// a saasadmin covering their tracks with delete leaves only the
/// tracing log line, which sits outside the DB. Changing the FK to
/// SET NULL is a separate schema decision.
async fn audit_saas(
    state: &Arc<AppState>,
    headers: &HeaderMap,
    ctx: &SessionContext,
    target_workspace_id: Uuid,
    action: &str,
    payload: Value,
) {
    let (ip, ua) = crate::notify::extract_request_meta(headers);
    crate::notify::audit(
        &state.pool,
        target_workspace_id,
        None,
        Some(ctx.user_id.into_uuid()),
        action,
        Some("workspace"),
        Some(&target_workspace_id.to_string()),
        crate::notify::enrich_payload(payload, ip.as_deref(), ua.as_deref()),
    )
    .await;
}

pub async fn workspaces(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query(
        "SELECT w.id, w.name, w.created_at, \
                COALESCE(wb.plan, 'free') AS plan, \
                COALESCE(wb.status, 'active') AS status, \
                COALESCE((SELECT COUNT(*) FROM projects WHERE workspace_id = w.id), 0) AS project_count, \
                COALESCE((SELECT COUNT(*) FROM workspace_members WHERE workspace_id = w.id), 0) AS member_count \
         FROM workspaces w \
         LEFT JOIN workspace_billing wb ON wb.workspace_id = w.id \
         ORDER BY w.created_at DESC LIMIT 500",
    )
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let out: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<Uuid, _>("id").to_string(),
                "name": r.get::<String, _>("name"),
                "created_at": crate::wire_time::rfc3339(r.get::<time::OffsetDateTime, _>("created_at")),
                "plan": r.get::<String, _>("plan"),
                "status": r.get::<String, _>("status"),
                "project_count": r.get::<i64, _>("project_count"),
                "member_count": r.get::<i64, _>("member_count"),
            })
        })
        .collect();
    Json(json!({ "workspaces": out }))
}

pub async fn workspace_stats(State(state): State<Arc<AppState>>) -> Json<Value> {
    // Aggregate counts across the deployment.
    let workspaces: i64 = sqlx::query("SELECT COUNT(*) AS n FROM workspaces")
        .fetch_one(&state.pool)
        .await
        .map_or_else(
            |e| {
                warn!(error = %e, "saas.workspace_stats workspaces query");
                0
            },
            |r| r.get("n"),
        );
    let active: i64 =
        sqlx::query("SELECT COUNT(*) AS n FROM workspace_billing WHERE status = 'active'")
            .fetch_one(&state.pool)
            .await
            .map_or(0, |r| r.get("n"));
    let projects: i64 = sqlx::query("SELECT COUNT(*) AS n FROM projects")
        .fetch_one(&state.pool)
        .await
        .map_or(0, |r| r.get("n"));
    let users: i64 = sqlx::query("SELECT COUNT(*) AS n FROM users")
        .fetch_one(&state.pool)
        .await
        .map_or(0, |r| r.get("n"));
    let events_24h: i64 = sqlx::query(
        "SELECT COUNT(*) AS n FROM events WHERE received_at >= now() - interval '24 hours'",
    )
    .fetch_one(&state.pool)
    .await
    .map_or(0, |r| r.get("n"));
    let tokens_active: i64 =
        sqlx::query("SELECT COUNT(*) AS n FROM tokens WHERE revoked_at IS NULL")
            .fetch_one(&state.pool)
            .await
            .map_or(0, |r| r.get("n"));
    Json(json!({
        "workspaces": workspaces,
        "active_workspaces": active,
        "projects": projects,
        "users": users,
        "events_24h": events_24h,
        "tokens_active": tokens_active,
    }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBody {
    pub name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateResponse {
    pub id: Uuid,
    pub name: String,
    pub status: String,
}

pub async fn create_workspace(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    headers: HeaderMap,
    Json(body): Json<CreateBody>,
) -> Result<(StatusCode, Json<CreateResponse>), (StatusCode, String)> {
    if body.name.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "name required".into()));
    }
    let id = Uuid::now_v7();

    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
        .bind(id)
        .bind(body.name.trim())
        .execute(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Bootstrap a billing row at "active" / "free" plan. Stripe
    // wiring lives in the saas-control webhook receiver; this is
    // the initial seed before any payment event.
    let billing_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO workspace_billing (id, workspace_id, plan, status) \
         VALUES ($1, $2, 'free', 'active') ON CONFLICT (workspace_id) DO NOTHING",
    )
    .bind(billing_id)
    .bind(id)
    .execute(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    info!(workspace_id = %id, name = %body.name, "saas.workspaces created");
    audit_saas(
        &state,
        &headers,
        &ctx,
        id,
        "saas.workspace.create",
        json!({ "name": body.name.trim() }),
    )
    .await;
    Ok((
        StatusCode::CREATED,
        Json(CreateResponse {
            id,
            name: body.name.trim().to_string(),
            status: "active".into(),
        }),
    ))
}

pub async fn delete_workspace(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
) -> StatusCode {
    // The audit row goes in before the DELETE. It will be cascaded
    // away when the workspace is dropped, but writing it first at
    // least means the intent hit the WAL and the tracing INFO line
    // has a matching id to correlate against externally-collected
    // logs. See `audit_saas` for the trade-off.
    audit_saas(
        &state,
        &headers,
        &ctx,
        workspace_id,
        "saas.workspace.delete",
        json!({}),
    )
    .await;

    // Hard delete cascades via FK (workspaces.id → projects.workspace_id
    // → events.workspace_id, etc. all ON DELETE CASCADE).
    let result = sqlx::query("DELETE FROM workspaces WHERE id = $1")
        .bind(workspace_id)
        .execute(&state.pool)
        .await;
    match result {
        Ok(_) => {
            info!(%workspace_id, "saas.workspaces deleted");
            StatusCode::NO_CONTENT
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

pub async fn suspend_workspace(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, String)> {
    // Operator kill-switch: set `canceled`, which the quota path
    // reads as Free-tier limits (see `sentori_billing::effective_plan`).
    // We deliberately do NOT use `past_due` here — that is Stripe's
    // dunning grace state, where quotas still apply, so a manual
    // suspend written as past_due would enforce nothing. `canceled`
    // is the state that actually bites; a later resume restores
    // `active` and the untouched `plan` column takes effect again.
    let res = sqlx::query(
        "UPDATE workspace_billing SET status = 'canceled', updated_at = now() \
         WHERE workspace_id = $1 AND status <> 'canceled'",
    )
    .bind(id)
    .execute(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if res.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            "workspace billing row already suspended / missing".into(),
        ));
    }
    audit_saas(
        &state,
        &headers,
        &ctx,
        id,
        "saas.workspace.suspend",
        json!({}),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct SetPlanBody {
    pub plan: String,
}

/// Operator override: force a workspace onto a plan (comped
/// enterprise, manual downgrade, …). Self-serve upgrades go
/// through Stripe Checkout + the webhook, not this — Stripe stays
/// the source of truth there. This is the out-of-band lever.
///
/// Also resets status to `active`: granting a plan implies it
/// should take effect, even if the workspace was previously
/// suspended / canceled.
pub async fn set_plan(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<SetPlanBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    let plan = Plan::from_db_str(body.plan.trim())
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let billing = state.billing_for(WorkspaceId::from_uuid(id));
    billing
        .set_plan(plan, None, None)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    billing
        .set_status(PlanStatus::Active)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    info!(workspace_id = %id, %plan, "saas.set_plan operator override");
    audit_saas(
        &state,
        &headers,
        &ctx,
        id,
        "saas.workspace.set_plan",
        json!({ "plan": plan.as_db_str() }),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn resume_workspace(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, String)> {
    let res = sqlx::query(
        "UPDATE workspace_billing SET status = 'active', updated_at = now() \
         WHERE workspace_id = $1 AND status <> 'active'",
    )
    .bind(id)
    .execute(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if res.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            "workspace billing row already active / missing".into(),
        ));
    }
    audit_saas(
        &state,
        &headers,
        &ctx,
        id,
        "saas.workspace.resume",
        json!({}),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}
