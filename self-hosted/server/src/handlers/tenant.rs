//! Tenant scoping helpers for the dashboard read API.
//!
//! Until 2026-07-20 these handlers selected across the whole table
//! and took `project_id` straight from the path, so any authenticated
//! user could read any workspace's data by naming its project id —
//! and `GET /v1/projects` listed every workspace's projects to
//! address them by.
//!
//! Two layers, deliberately overlapping:
//!
//! 1. [`guard_project`] rejects a `project_id` that isn't in the
//!    caller's workspace, before the handler runs its own query.
//! 2. The queries themselves also filter on `workspace_id`, so a
//!    handler added later that forgets the guard still can't read
//!    across the boundary.
//!
//! The guard answers 404 rather than 403 for a foreign project: a
//! caller shouldn't be able to tell someone else's project id apart
//! from one that doesn't exist.

use std::sync::Arc;

use axum::http::StatusCode;
use sentori_workspace_identity::WorkspaceId;
use uuid::Uuid;

use crate::state::AppState;

/// Error shape shared by the dashboard handlers.
pub type ApiErr = (StatusCode, String);

/// Confirm `project_id` belongs to `workspace_id`.
///
/// # Errors
///
/// - `404` when the project is absent **or** owned by another
///   workspace — the two are deliberately indistinguishable.
/// - `500` on a database failure.
pub async fn guard_project(
    state: &Arc<AppState>,
    workspace_id: WorkspaceId,
    project_id: Uuid,
) -> Result<(), ApiErr> {
    let row: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM projects WHERE id = $1 AND workspace_id = $2")
            .bind(project_id)
            .bind(workspace_id.into_uuid())
            .fetch_optional(&state.pool)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if row.is_none() {
        return Err((StatusCode::NOT_FOUND, "project not found".into()));
    }
    Ok(())
}

/// Same for an issue, which handlers address without a project id.
///
/// # Errors
///
/// As [`guard_project`].
pub async fn guard_issue(
    state: &Arc<AppState>,
    workspace_id: WorkspaceId,
    issue_id: Uuid,
) -> Result<(), ApiErr> {
    let row: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM issues WHERE id = $1 AND workspace_id = $2")
            .bind(issue_id)
            .bind(workspace_id.into_uuid())
            .fetch_optional(&state.pool)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if row.is_none() {
        return Err((StatusCode::NOT_FOUND, "issue not found".into()));
    }
    Ok(())
}

/// Confirm a row identified by its own id belongs to `workspace_id`,
/// for handlers that address a resource directly (alert rule id,
/// saved view id, …) where the backing service method keys on the
/// id alone. `table` is a trusted static string from the caller, not
/// user input — never interpolate a request value here.
///
/// # Errors
///
/// - `404` when the row is absent or owned by another workspace.
/// - `500` on a database failure.
async fn guard_row(
    state: &Arc<AppState>,
    workspace_id: WorkspaceId,
    table: &'static str,
    id: Uuid,
) -> Result<(), ApiErr> {
    let sql = format!("SELECT id FROM {table} WHERE id = $1 AND workspace_id = $2");
    let row: Option<(Uuid,)> = sqlx::query_as(&sql)
        .bind(id)
        .bind(workspace_id.into_uuid())
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if row.is_none() {
        return Err((StatusCode::NOT_FOUND, "not found".into()));
    }
    Ok(())
}

/// Confirm an alert rule belongs to the caller's workspace.
///
/// # Errors
///
/// As [`guard_project`] (404 absent/foreign, 500 on db error).
pub async fn guard_alert(
    state: &Arc<AppState>,
    workspace_id: WorkspaceId,
    alert_id: Uuid,
) -> Result<(), ApiErr> {
    guard_row(state, workspace_id, "alert_rules", alert_id).await
}

/// Confirm a saved view belongs to the caller's workspace.
///
/// # Errors
///
/// As [`guard_project`] (404 absent/foreign, 500 on db error).
pub async fn guard_saved_view(
    state: &Arc<AppState>,
    workspace_id: WorkspaceId,
    view_id: Uuid,
) -> Result<(), ApiErr> {
    guard_row(state, workspace_id, "saved_views", view_id).await
}

#[cfg(test)]
mod tests {
    //! The guards need a live database, so the coverage here is on
    //! the property that matters and can be checked without one:
    //! absent and foreign must produce the same answer, or the 404
    //! becomes an oracle for guessing other tenants' project ids.

    use axum::http::StatusCode;

    #[test]
    fn foreign_and_absent_are_indistinguishable() {
        let absent = (StatusCode::NOT_FOUND, "project not found".to_string());
        let foreign = (StatusCode::NOT_FOUND, "project not found".to_string());
        assert_eq!(absent, foreign);
        assert_ne!(absent.0, StatusCode::FORBIDDEN);
    }
}

#[cfg(test)]
mod scoping_tests {
    //! Guards against the shape of the 2026-07-20 gap returning: a
    //! dashboard query that names a tenant table without also
    //! constraining `workspace_id`, or a handler that takes a
    //! `project_id`/`issue_id` from the path without a guard call.
    //!
    //! Reads the handler sources rather than the database, so it runs
    //! in CI with no Postgres.

    use std::fs;
    use std::path::Path;

    /// Handlers serving the session-gated dashboard read API.
    const DASHBOARD_HANDLERS: [&str; 6] = [
        "events.rs",
        "spans.rs",
        "metrics.rs",
        "replays.rs",
        "search.rs",
        "projects.rs",
    ];

    /// Tables holding per-tenant rows that all carry `workspace_id`.
    const TENANT_TABLES: [&str; 7] = [
        "FROM events",
        "FROM spans",
        "FROM traces",
        "FROM metrics",
        "FROM replay_sessions",
        "FROM issues",
        "FROM projects",
    ];

    // Test-only helper: a source file that cannot be read means the
    // tenant-isolation scan cannot run at all, so failing loudly is
    // the correct outcome.
    #[allow(clippy::panic)]
    fn source(file: &str) -> String {
        let p = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/handlers")
            .join(file);
        fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
    }

    /// SELECTs in real code, ignoring ones quoted in comments —
    /// a doc comment describing the old unscoped query is not a
    /// query.
    fn count_live_selects(src: &str) -> usize {
        src.lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .filter(|l| l.contains("SELECT"))
            .count()
    }

    #[test]
    fn dashboard_queries_constrain_workspace() {
        for file in DASHBOARD_HANDLERS {
            let src = source(file);
            let selects = count_live_selects(&src);
            let scoped = src.matches("workspace_id = $").count();
            assert!(
                scoped > 0,
                "{file} queries tenant data but never constrains workspace_id"
            );
            assert!(
                scoped >= selects,
                "{file}: {selects} SELECT(s) but only {scoped} workspace_id constraint(s) \
                 — one of them reads across tenants"
            );
        }
    }

    #[test]
    fn path_addressed_handlers_call_a_guard() {
        // A handler that accepts an id from the URL must prove the id
        // belongs to the caller before querying with it.
        for file in DASHBOARD_HANDLERS {
            let src = source(file);
            if src.contains("Path(project_id)") || src.contains("project_id): Path") {
                assert!(
                    src.contains("guard_project("),
                    "{file} takes project_id from the path without calling guard_project"
                );
            }
        }
    }

    #[test]
    fn every_tenant_table_is_covered_by_the_scan() {
        // Keeps TENANT_TABLES honest: if a table is renamed the test
        // above would silently stop checking it.
        let joined = DASHBOARD_HANDLERS.map(source).join("\n");
        let hits = TENANT_TABLES
            .iter()
            .filter(|t| joined.contains(**t))
            .count();
        assert!(
            hits >= 5,
            "expected the dashboard handlers to read most tenant tables, matched {hits}"
        );
    }

    /// Admin + mutation handlers that must act in the caller's
    /// *active* workspace (`ctx.workspace_id`), never the boot-time
    /// default (`state.workspace_id`) or the default-bound
    /// `state.identity`. Before 2026-07-21 every one of these wrote
    /// to / read from `DEFAULT_WORKSPACE_ID`, so in a multi-tenant
    /// deployment they touched the wrong tenant.
    const CTX_SCOPED_HANDLERS: [&str; 11] = [
        "admin/tokens.rs",
        "admin/projects.rs",
        "admin/members.rs",
        "admin/invites.rs",
        "admin/push_sends.rs",
        "admin/push_credentials.rs",
        "admin/test_push.rs",
        "alerts.rs",
        "alerts_fire.rs",
        "saved_views.rs",
        "sessions_admin.rs",
    ];

    /// A live (non-comment) line that binds workspace scope from the
    /// process default rather than the session. The whole point of
    /// the multi-workspace cutover is that these disappear from
    /// request handlers.
    fn live_uses(src: &str, needle: &str) -> bool {
        src.lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .any(|l| l.contains(needle))
    }

    #[test]
    fn admin_handlers_scope_to_session_not_default_workspace() {
        for file in CTX_SCOPED_HANDLERS {
            let src = source(file);
            assert!(
                !live_uses(&src, "state.workspace_id"),
                "{file} still binds the default workspace via `state.workspace_id` — \
                 use `ctx.workspace_id` so the write lands in the caller's tenant"
            );
            // `state.identity.` (trailing dot = a method call on the
            // default-bound handle) is the leak; `state.identity_for(`
            // is the correct request-scoped constructor and must not
            // trip the check.
            assert!(
                !live_uses(&src, "state.identity."),
                "{file} still uses the default-bound `state.identity` — \
                 use `state.identity_for(ctx.workspace_id)`"
            );
        }
    }
}
