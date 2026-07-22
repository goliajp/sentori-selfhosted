//! `project_user_visibility` table CRUD.
//!
//! Rows here only exist for users with `Role::User`. Owners and
//! admins auto-see every project; granting them visibility is a
//! logic error (caught explicitly — see
//! [`crate::IdentityError::VisibilityRefusedForElevatedRole`]).

use sqlx::{PgPool, Row};

use crate::WorkspaceId;
use crate::error::IdentityError;
use crate::model::{ProjectId, Role, UserId};

/// Store sub-handle for `project_user_visibility`. Workspace-scoped.
#[derive(Debug, Clone, Copy)]
pub struct Visibility<'a> {
    pool: &'a PgPool,
    workspace_id: WorkspaceId,
}

impl<'a> Visibility<'a> {
    /// Construct over a borrowed pool + workspace id.
    #[must_use]
    pub const fn new(pool: &'a PgPool, workspace_id: WorkspaceId) -> Self {
        Self { pool, workspace_id }
    }

    /// Grant `user_id` visibility on `project_id` within this
    /// workspace.
    ///
    /// # Errors
    ///
    /// - [`IdentityError::NotAMember`] if `user_id` is not in
    ///   `workspace_members` for this workspace.
    /// - [`IdentityError::VisibilityRefusedForElevatedRole`]
    ///   if the user is owner or admin.
    /// - [`IdentityError::ProjectNotFound`] if `project_id`
    ///   does not exist in this workspace.
    /// - [`IdentityError::Db`] on database failure.
    pub async fn grant(
        &self,
        project_id: ProjectId,
        user_id: UserId,
        granted_by: UserId,
    ) -> Result<(), IdentityError> {
        let role = lookup_role(self.pool, self.workspace_id, user_id).await?;
        if role.auto_sees_all_projects() {
            return Err(IdentityError::VisibilityRefusedForElevatedRole);
        }

        sqlx::query(
            "INSERT INTO project_user_visibility \
             (workspace_id, project_id, user_id, granted_by) \
             VALUES ($1, $2, $3, $4) \
             ON CONFLICT (project_id, user_id) DO NOTHING",
        )
        .bind(self.workspace_id.into_uuid())
        .bind(project_id.into_uuid())
        .bind(user_id.into_uuid())
        .bind(granted_by.into_uuid())
        .execute(self.pool)
        .await
        .map_err(|e| translate_project_fk(e, project_id))?;

        Ok(())
    }

    /// Revoke `user_id`'s visibility on `project_id`.
    ///
    /// # Errors
    ///
    /// [`IdentityError::Db`] on database failure.
    pub async fn revoke(
        &self,
        project_id: ProjectId,
        user_id: UserId,
    ) -> Result<(), IdentityError> {
        sqlx::query(
            "DELETE FROM project_user_visibility \
             WHERE workspace_id = $1 AND project_id = $2 AND user_id = $3",
        )
        .bind(self.workspace_id.into_uuid())
        .bind(project_id.into_uuid())
        .bind(user_id.into_uuid())
        .execute(self.pool)
        .await?;
        Ok(())
    }

    /// List user ids granted visibility on `project_id`.
    ///
    /// # Errors
    ///
    /// [`IdentityError::Db`] on database failure.
    pub async fn list_for_project(
        &self,
        project_id: ProjectId,
    ) -> Result<Vec<UserId>, IdentityError> {
        let rows = sqlx::query(
            "SELECT user_id FROM project_user_visibility \
             WHERE workspace_id = $1 AND project_id = $2 \
             ORDER BY granted_at ASC",
        )
        .bind(self.workspace_id.into_uuid())
        .bind(project_id.into_uuid())
        .fetch_all(self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|r| UserId::from_uuid(r.get::<uuid::Uuid, _>("user_id")))
            .collect())
    }

    /// List project ids `user_id` has been explicitly granted in
    /// this workspace.
    ///
    /// # Errors
    ///
    /// [`IdentityError::Db`] on database failure.
    pub async fn list_for_user(&self, user_id: UserId) -> Result<Vec<ProjectId>, IdentityError> {
        let rows = sqlx::query(
            "SELECT project_id FROM project_user_visibility \
             WHERE workspace_id = $1 AND user_id = $2 \
             ORDER BY granted_at ASC",
        )
        .bind(self.workspace_id.into_uuid())
        .bind(user_id.into_uuid())
        .fetch_all(self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|r| ProjectId::from_uuid(r.get::<uuid::Uuid, _>("project_id")))
            .collect())
    }
}

async fn lookup_role(
    pool: &PgPool,
    workspace_id: WorkspaceId,
    user_id: UserId,
) -> Result<Role, IdentityError> {
    let row = sqlx::query(
        "SELECT role FROM workspace_members \
         WHERE workspace_id = $1 AND user_id = $2",
    )
    .bind(workspace_id.into_uuid())
    .bind(user_id.into_uuid())
    .fetch_optional(pool)
    .await?;
    let role_str: &str = row
        .as_ref()
        .map(|r| r.get::<&str, _>("role"))
        .ok_or(IdentityError::NotAMember(user_id))?;
    Role::from_db_str(role_str).map_err(IdentityError::InvalidRoleInDatabase)
}

fn translate_project_fk(err: sqlx::Error, project_id: ProjectId) -> IdentityError {
    if let sqlx::Error::Database(db_err) = &err
        && db_err.code().as_deref() == Some("23503")
    {
        return IdentityError::ProjectNotFound(project_id.into_uuid());
    }
    IdentityError::Db(err)
}
