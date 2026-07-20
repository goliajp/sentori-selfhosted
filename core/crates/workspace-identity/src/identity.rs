//! [`Identity`] — the one handle a caller constructs, fanning
//! out into five typed sub-handles, all scoped to a fixed
//! [`WorkspaceId`].

use sqlx::PgPool;

use crate::WorkspaceId;
use crate::store::{Invites, Members, Projects, Users, Visibility};

/// Workspace-scoped handle into the identity tables.
///
/// Holds a `PgPool` clone + a fixed [`WorkspaceId`]. The five
/// sub-handles inherit both — every CRUD call automatically
/// filters / writes against `workspace_id`. There is no API
/// surface for cross-workspace queries; create one [`Identity`]
/// per workspace context.
///
/// ## Bootstrapping a new workspace
///
/// Use the free function `bootstrap_workspace(&pool, name)` (in
/// the crate root) to INSERT the `workspaces` row, then
/// construct [`Identity::new`] against the returned id.
///
/// ## Pool routing for saas
///
/// In `SaaS` deployments the per-request middleware should set the
/// `app.current_workspace` GUC on the borrowed connection so the
/// RLS policies (see migration 0001) match. The store methods
/// here also explicitly bind `workspace_id` in WHERE clauses /
/// INSERT lists — RLS is defense-in-depth, not the only
/// boundary.
#[derive(Debug, Clone)]
pub struct Identity {
    pool: PgPool,
    workspace_id: WorkspaceId,
}

impl Identity {
    /// Wrap a pool against a specific workspace.
    #[must_use]
    pub const fn new(pool: PgPool, workspace_id: WorkspaceId) -> Self {
        Self { pool, workspace_id }
    }

    /// Borrow the underlying pool for callers that need to run
    /// their own ad-hoc queries against the same connection.
    /// Prefer the sub-handle methods.
    #[must_use]
    pub const fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// The workspace this handle is scoped to.
    #[must_use]
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    /// CRUD on `users`.
    #[must_use]
    pub const fn users(&self) -> Users<'_> {
        Users::new(&self.pool, self.workspace_id)
    }

    /// CRUD on `workspace_members` (RBAC: owner / admin / user).
    #[must_use]
    pub const fn members(&self) -> Members<'_> {
        Members::new(&self.pool, self.workspace_id)
    }

    /// CRUD on `projects` (+ owning `privacy_salts`).
    #[must_use]
    pub const fn projects(&self) -> Projects<'_> {
        Projects::new(&self.pool, self.workspace_id)
    }

    /// CRUD on `project_user_visibility` (per-project ACL for
    /// `Role::User`).
    #[must_use]
    pub const fn visibility(&self) -> Visibility<'_> {
        Visibility::new(&self.pool, self.workspace_id)
    }

    /// CRUD on `workspace_invites`.
    #[must_use]
    pub const fn invites(&self) -> Invites<'_> {
        Invites::new(&self.pool, self.workspace_id)
    }
}

/// Bootstrap a new workspace row. Used at server startup
/// (self-hosted's default workspace) and by `SaaS` signup flow.
///
/// Returns the new [`WorkspaceId`]; immediately usable as
/// argument to [`Identity::new`].
///
/// # Errors
///
/// Forwards any sqlx error.
pub async fn bootstrap_workspace(pool: &PgPool, name: &str) -> Result<WorkspaceId, sqlx::Error> {
    let id = WorkspaceId::new();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
        .bind(id.into_uuid())
        .bind(name)
        .execute(pool)
        .await?;
    Ok(id)
}

/// Ensure a workspace exists with the given id (idempotent). The
/// `SaaS` signup flow uses this to round-trip through transactions
/// where the workspace id is minted before the row commits.
///
/// # Errors
///
/// Forwards any sqlx error.
pub async fn ensure_workspace(
    pool: &PgPool,
    id: WorkspaceId,
    name: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO workspaces (id, name) VALUES ($1, $2) \
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(id.into_uuid())
    .bind(name)
    .execute(pool)
    .await?;
    Ok(())
}
