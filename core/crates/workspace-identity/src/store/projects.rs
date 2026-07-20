//! `projects` (and owning `privacy_salts`) table CRUD.

use sqlx::{PgPool, Row};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::WorkspaceId;
use crate::error::IdentityError;
use crate::model::{Project, ProjectId, UserId};

/// Store sub-handle for `projects`. Workspace-scoped.
#[derive(Debug, Clone, Copy)]
pub struct Projects<'a> {
    pool: &'a PgPool,
    workspace_id: WorkspaceId,
}

impl<'a> Projects<'a> {
    /// Construct over a borrowed pool + workspace id.
    #[must_use]
    pub const fn new(pool: &'a PgPool, workspace_id: WorkspaceId) -> Self {
        Self { pool, workspace_id }
    }

    /// Create a project together with a fresh `privacy_salts`
    /// row owning its salt. The two writes happen in one
    /// transaction.
    ///
    /// `salt_bytes` must be 32 bytes (mint via
    /// `sentori_privacy_salt::Salt::generate()` in the caller
    /// layer). We accept arbitrary length here; the
    /// salt-generation crate enforces the length on its end.
    ///
    /// # Errors
    ///
    /// - [`IdentityError::SlugTaken`] if `slug` already exists
    ///   in this workspace.
    /// - [`IdentityError::Db`] on database failure.
    pub async fn create(
        &self,
        name: &str,
        slug: &str,
        salt_bytes: &[u8],
    ) -> Result<Project, IdentityError> {
        let mut tx = self.pool.begin().await?;

        let salt_id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO privacy_salts (id, workspace_id, salt_bytes) \
             VALUES ($1, $2, $3)",
        )
        .bind(salt_id)
        .bind(self.workspace_id.into_uuid())
        .bind(salt_bytes)
        .execute(&mut *tx)
        .await?;

        let project_id = ProjectId::new();
        let row = sqlx::query(
            "INSERT INTO projects (id, workspace_id, name, slug, privacy_salt_id) \
             VALUES ($1, $2, $3, $4, $5) \
             RETURNING id, name, slug, privacy_salt_id, created_at",
        )
        .bind(project_id.into_uuid())
        .bind(self.workspace_id.into_uuid())
        .bind(name)
        .bind(slug)
        .bind(salt_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| translate_slug_taken(e, slug))?;

        tx.commit().await?;

        Ok(row_to_project(&row))
    }

    /// Look up a project by id, scoped to this workspace.
    ///
    /// # Errors
    ///
    /// [`IdentityError::Db`] on database failure.
    pub async fn find(&self, id: ProjectId) -> Result<Option<Project>, IdentityError> {
        let row = sqlx::query(
            "SELECT id, name, slug, privacy_salt_id, created_at \
             FROM projects WHERE id = $1 AND workspace_id = $2",
        )
        .bind(id.into_uuid())
        .bind(self.workspace_id.into_uuid())
        .fetch_optional(self.pool)
        .await?;

        Ok(row.as_ref().map(row_to_project))
    }

    /// Look up a project by slug within this workspace.
    ///
    /// # Errors
    ///
    /// [`IdentityError::Db`] on database failure.
    pub async fn find_by_slug(&self, slug: &str) -> Result<Option<Project>, IdentityError> {
        let row = sqlx::query(
            "SELECT id, name, slug, privacy_salt_id, created_at \
             FROM projects WHERE slug = $1 AND workspace_id = $2",
        )
        .bind(slug)
        .bind(self.workspace_id.into_uuid())
        .fetch_optional(self.pool)
        .await?;

        Ok(row.as_ref().map(row_to_project))
    }

    /// All projects in the workspace, ordered by `created_at`
    /// ascending.
    ///
    /// # Errors
    ///
    /// [`IdentityError::Db`] on database failure.
    pub async fn list_all(&self) -> Result<Vec<Project>, IdentityError> {
        let rows = sqlx::query(
            "SELECT id, name, slug, privacy_salt_id, created_at \
             FROM projects WHERE workspace_id = $1 ORDER BY created_at ASC",
        )
        .bind(self.workspace_id.into_uuid())
        .fetch_all(self.pool)
        .await?;
        Ok(rows.iter().map(row_to_project).collect())
    }

    /// Projects in this workspace visible to `user_id`.
    ///
    /// # Errors
    ///
    /// [`IdentityError::Db`] on database failure.
    pub async fn list_visible_to(&self, user_id: UserId) -> Result<Vec<Project>, IdentityError> {
        let rows = sqlx::query(
            "SELECT p.id, p.name, p.slug, p.privacy_salt_id, p.created_at \
             FROM projects p \
             JOIN workspace_members wm \
                ON wm.workspace_id = $1 AND wm.user_id = $2 \
             WHERE p.workspace_id = $1 \
               AND ( wm.role IN ('owner','admin') \
                  OR EXISTS ( \
                       SELECT 1 FROM project_user_visibility v \
                       WHERE v.project_id = p.id AND v.user_id = $2 \
                  ) \
                  ) \
             ORDER BY p.created_at ASC",
        )
        .bind(self.workspace_id.into_uuid())
        .bind(user_id.into_uuid())
        .fetch_all(self.pool)
        .await?;

        Ok(rows.iter().map(row_to_project).collect())
    }

    /// Delete a project from this workspace.
    ///
    /// # Errors
    ///
    /// - [`IdentityError::ProjectNotFound`] if `id` does not
    ///   exist in this workspace.
    /// - [`IdentityError::Db`] on database failure.
    pub async fn delete(&self, id: ProjectId) -> Result<(), IdentityError> {
        let result = sqlx::query("DELETE FROM projects WHERE id = $1 AND workspace_id = $2")
            .bind(id.into_uuid())
            .bind(self.workspace_id.into_uuid())
            .execute(self.pool)
            .await?;
        if result.rows_affected() == 0 {
            Err(IdentityError::ProjectNotFound(id.into_uuid()))
        } else {
            Ok(())
        }
    }
}

fn row_to_project(row: &sqlx::postgres::PgRow) -> Project {
    Project {
        id: ProjectId::from_uuid(row.get("id")),
        name: row.get("name"),
        slug: row.get("slug"),
        privacy_salt_id: row.get("privacy_salt_id"),
        created_at: row.get::<OffsetDateTime, _>("created_at"),
    }
}

fn translate_slug_taken(err: sqlx::Error, slug: &str) -> IdentityError {
    if let sqlx::Error::Database(db_err) = &err
        && db_err.code().as_deref() == Some("23505")
        && db_err
            .constraint()
            .is_none_or(|c| c == "projects_workspace_slug_idx" || c == "projects_slug_key")
    {
        return IdentityError::SlugTaken(slug.to_string());
    }
    IdentityError::Db(err)
}
