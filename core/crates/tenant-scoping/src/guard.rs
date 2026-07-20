//! [`TenantGuard`] — the public handle.

use sentori_workspace_identity::{Identity, ProjectId, Role, UserId, WorkspaceId};
use sqlx::PgPool;

use crate::error::TenantError;
use crate::permission::{Permission, role_allows};

/// Public handle. Carries the workspace context so every
/// permission decision is scoped to one workspace.
#[derive(Clone, Debug)]
pub struct TenantGuard {
    pool: PgPool,
    workspace_id: WorkspaceId,
}

impl TenantGuard {
    /// Construct with the workspace context.
    #[must_use]
    pub const fn new(pool: PgPool, workspace_id: WorkspaceId) -> Self {
        Self { pool, workspace_id }
    }

    /// Borrow the pool.
    #[must_use]
    pub const fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// The workspace this guard is scoped to.
    #[must_use]
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    fn identity(&self) -> Identity {
        Identity::new(self.pool.clone(), self.workspace_id)
    }

    // ── role lookup ─────────────────────────────────────────

    /// Look up a user's workspace role. Returns `None` when
    /// the user isn't a workspace member at all.
    ///
    /// # Errors
    ///
    /// [`TenantError::Db`] on backend failure.
    pub async fn member_role(&self, user: UserId) -> Result<Option<Role>, TenantError> {
        let member = self
            .identity()
            .members()
            .find(user)
            .await
            .map_err(map_identity_err)?;
        Ok(member.map(|m| m.role))
    }

    // ── project visibility ──────────────────────────────────

    /// True when `user` can see `project`. Owner / Admin
    /// always yes; User-role checks `project_user_visibility`.
    /// Non-members get `Ok(false)` (no error — the question
    /// is "can they see it?", and the answer is no).
    ///
    /// # Errors
    ///
    /// [`TenantError::Db`] on backend failure.
    pub async fn can_view_project(
        &self,
        user: UserId,
        project: ProjectId,
    ) -> Result<bool, TenantError> {
        match self.member_role(user).await? {
            None => Ok(false),
            Some(Role::Owner | Role::Admin) => Ok(true),
            Some(Role::User) => self.has_visibility_row(user, project).await,
        }
    }

    /// Every project visible to `user`. For Owners / Admins
    /// this returns every project in the workspace; for
    /// Users it returns the rows in
    /// `project_user_visibility`. Non-members get an empty
    /// vec (not an error).
    ///
    /// # Errors
    ///
    /// [`TenantError::Db`] on backend failure.
    pub async fn visible_projects(&self, user: UserId) -> Result<Vec<ProjectId>, TenantError> {
        match self.member_role(user).await? {
            None => Ok(Vec::new()),
            Some(Role::Owner | Role::Admin) => {
                let rows: Vec<(uuid::Uuid,)> = sqlx::query_as(
                    "SELECT id FROM projects \
                     WHERE workspace_id = $1 ORDER BY created_at ASC",
                )
                .bind(self.workspace_id.into_uuid())
                .fetch_all(&self.pool)
                .await?;
                Ok(rows
                    .into_iter()
                    .map(|(u,)| ProjectId::from_uuid(u))
                    .collect())
            }
            Some(Role::User) => self
                .identity()
                .visibility()
                .list_for_user(user)
                .await
                .map_err(map_identity_err),
        }
    }

    // ── question API ────────────────────────────────────────

    /// True when `user` may perform `permission` against
    /// `project`. Composes the role gate
    /// ([`role_allows`]) + project visibility for project-
    /// scoped permissions on User-role.
    ///
    /// # Errors
    ///
    /// [`TenantError::Db`] on backend failure.
    pub async fn can_perform(
        &self,
        user: UserId,
        project: ProjectId,
        permission: Permission,
    ) -> Result<bool, TenantError> {
        let Some(role) = self.member_role(user).await? else {
            return Ok(false);
        };
        if !role_allows(role, permission) {
            return Ok(false);
        }
        if matches!(role, Role::User) && permission.is_project_scoped() {
            return self.has_visibility_row(user, project).await;
        }
        Ok(true)
    }

    // ── assert API ──────────────────────────────────────────

    /// Returns Ok when `user` can perform; Err with the
    /// specific denial reason otherwise. Endpoints prefer
    /// this over `can_perform` so the error type already
    /// matches their `?`-propagation shape.
    ///
    /// # Errors
    ///
    /// [`TenantError::NotAMember`] /
    /// [`TenantError::InsufficientRole`] /
    /// [`TenantError::NotVisible`] / [`TenantError::Db`].
    pub async fn assert_can_perform(
        &self,
        user: UserId,
        project: ProjectId,
        permission: Permission,
    ) -> Result<(), TenantError> {
        let role = self
            .member_role(user)
            .await?
            .ok_or_else(|| TenantError::NotAMember(user.into_uuid()))?;
        if !role_allows(role, permission) {
            return Err(TenantError::InsufficientRole { role, permission });
        }
        if matches!(role, Role::User)
            && permission.is_project_scoped()
            && !self.has_visibility_row(user, project).await?
        {
            return Err(TenantError::NotVisible {
                user: user.into_uuid(),
                project: project.into_uuid(),
            });
        }
        Ok(())
    }

    /// Convenience — `assert_can_perform(_, _, Permission::ViewProject)`.
    ///
    /// # Errors
    ///
    /// Same as [`Self::assert_can_perform`].
    pub async fn assert_can_view_project(
        &self,
        user: UserId,
        project: ProjectId,
    ) -> Result<(), TenantError> {
        self.assert_can_perform(user, project, Permission::ViewProject)
            .await
    }

    // ── visibility CRUD (admin-gated) ───────────────────────

    /// Grant `target_user` visibility on `project`. The
    /// `actor` is verified as Owner or Admin first; Users
    /// + non-members get `InsufficientRole` denial.
    ///
    /// Idempotent on (project, user). The `actor` is also
    /// recorded as `granted_by` for audit.
    ///
    /// # Errors
    ///
    /// Same as [`Self::assert_can_perform`] for `ManageMembers`,
    /// plus the underlying K1 visibility errors mapped to
    /// [`TenantError::Db`].
    pub async fn grant_visibility(
        &self,
        actor: UserId,
        target_user: UserId,
        project: ProjectId,
    ) -> Result<(), TenantError> {
        let role = self
            .member_role(actor)
            .await?
            .ok_or_else(|| TenantError::NotAMember(actor.into_uuid()))?;
        if !role_allows(role, Permission::ManageMembers) {
            return Err(TenantError::InsufficientRole {
                role,
                permission: Permission::ManageMembers,
            });
        }
        self.identity()
            .visibility()
            .grant(project, target_user, actor)
            .await
            .map_err(map_identity_err)
    }

    /// Revoke `target_user`'s visibility on `project`.
    /// Same role gate as [`Self::grant_visibility`].
    ///
    /// # Errors
    ///
    /// Same as [`Self::grant_visibility`].
    pub async fn revoke_visibility(
        &self,
        actor: UserId,
        target_user: UserId,
        project: ProjectId,
    ) -> Result<(), TenantError> {
        let role = self
            .member_role(actor)
            .await?
            .ok_or_else(|| TenantError::NotAMember(actor.into_uuid()))?;
        if !role_allows(role, Permission::ManageMembers) {
            return Err(TenantError::InsufficientRole {
                role,
                permission: Permission::ManageMembers,
            });
        }
        self.identity()
            .visibility()
            .revoke(project, target_user)
            .await
            .map_err(map_identity_err)
    }

    // ── internals ───────────────────────────────────────────

    async fn has_visibility_row(
        &self,
        user: UserId,
        project: ProjectId,
    ) -> Result<bool, TenantError> {
        let row: Option<(bool,)> = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM project_user_visibility \
             WHERE workspace_id = $1 AND user_id = $2 AND project_id = $3)",
        )
        .bind(self.workspace_id.into_uuid())
        .bind(user.into_uuid())
        .bind(project.into_uuid())
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.is_some_and(|(b,)| b))
    }
}

/// Map K1 [`sentori_workspace_identity::IdentityError`] into
/// [`TenantError::Db`]. K1 errors don't carry enough info
/// to discriminate further at this layer — the API surface
/// (`grant_visibility`) has already validated the FK exists
/// via the role lookup.
fn map_identity_err(err: sentori_workspace_identity::IdentityError) -> TenantError {
    use sentori_workspace_identity::IdentityError;
    match err {
        IdentityError::Db(e) => TenantError::Db(e),
        other => TenantError::Db(sqlx::Error::Protocol(other.to_string())),
    }
}
