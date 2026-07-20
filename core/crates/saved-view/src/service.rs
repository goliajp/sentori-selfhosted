//! [`SavedViewService`] — CRUD + visibility-aware listing.

use sentori_workspace_identity::{ProjectId, UserId};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::SavedViewError;
use crate::model::{SavedView, SavedViewDraft, SavedViewPatch, Scope, Target, row_to_view};

const MAX_NAME_LEN: usize = 200;

const SELECT_COLS: &str = r"
    id, project_id, target, scope, user_id, name,
    payload, created_at, created_by, updated_at
";

/// Public handle.
#[derive(Clone, Debug)]
pub struct SavedViewService {
    pool: PgPool,
}

impl SavedViewService {
    /// Construct.
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Borrow the pool.
    #[must_use]
    pub const fn pool(&self) -> &PgPool {
        &self.pool
    }

    // ── create ──────────────────────────────────────────────

    /// Insert a new saved view. Returns the row id.
    ///
    /// # Errors
    ///
    /// - [`SavedViewError::InvalidInput`] for empty / oversize
    ///   name, scope-FK polarity violation (Personal without
    ///   user_id, Workspace with user_id).
    /// - [`SavedViewError::ProjectNotFound`] /
    ///   [`SavedViewError::UserNotFound`] on FK fail.
    /// - [`SavedViewError::Db`].
    pub async fn create(&self, draft: SavedViewDraft) -> Result<Uuid, SavedViewError> {
        validate_draft(&draft)?;
        let id = Uuid::now_v7();
        let row: (Uuid,) = sqlx::query_as(
            r"
            INSERT INTO saved_views
                (id, workspace_id, project_id, target, scope, user_id, name, payload, created_by)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id
            ",
        )
        .bind(id)
        .bind(draft.workspace_id.into_uuid())
        .bind(draft.project_id.map(ProjectId::into_uuid))
        .bind(draft.target.as_db_str())
        .bind(draft.scope.as_db_str())
        .bind(draft.user_id.map(UserId::into_uuid))
        .bind(&draft.name)
        .bind(&draft.payload)
        .bind(draft.created_by.map(UserId::into_uuid))
        .fetch_one(&self.pool)
        .await
        .map_err(|e| translate_fk(e, draft.project_id, draft.user_id, draft.created_by))?;
        Ok(row.0)
    }

    // ── read ────────────────────────────────────────────────

    /// Look up by id.
    ///
    /// # Errors
    ///
    /// [`SavedViewError::Db`] on backend failure.
    pub async fn find(&self, id: Uuid) -> Result<Option<SavedView>, SavedViewError> {
        let sql = format!("SELECT {SELECT_COLS} FROM saved_views WHERE id = $1");
        let row = sqlx::query(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(row_to_view).transpose()
    }

    /// All views visible to `viewer` for `target`. Combines
    /// workspace-wide views (visible to everyone) + the
    /// viewer's personal views. Optional `project_id` filter
    /// — when set, only views that match that project OR
    /// workspace-wide views are returned.
    ///
    /// Sorted by `created_at` ascending.
    ///
    /// # Errors
    ///
    /// [`SavedViewError::Db`] on backend failure.
    pub async fn list_visible_to(
        &self,
        viewer: UserId,
        project_id: Option<ProjectId>,
        target: Target,
    ) -> Result<Vec<SavedView>, SavedViewError> {
        let sql = match project_id {
            Some(_) => format!(
                "SELECT {SELECT_COLS} FROM saved_views \
                 WHERE target = $1 \
                   AND (project_id = $2 OR project_id IS NULL) \
                   AND (scope = 'workspace' OR (scope = 'personal' AND user_id = $3)) \
                 ORDER BY created_at ASC"
            ),
            None => format!(
                "SELECT {SELECT_COLS} FROM saved_views \
                 WHERE target = $1 \
                   AND (scope = 'workspace' OR (scope = 'personal' AND user_id = $2)) \
                 ORDER BY created_at ASC"
            ),
        };
        let rows = match project_id {
            Some(pid) => {
                sqlx::query(&sql)
                    .bind(target.as_db_str())
                    .bind(pid.into_uuid())
                    .bind(viewer.into_uuid())
                    .fetch_all(&self.pool)
                    .await?
            }
            None => {
                sqlx::query(&sql)
                    .bind(target.as_db_str())
                    .bind(viewer.into_uuid())
                    .fetch_all(&self.pool)
                    .await?
            }
        };
        rows.iter().map(row_to_view).collect()
    }

    /// Personal views for one user, scoped by target.
    ///
    /// # Errors
    ///
    /// [`SavedViewError::Db`] on backend failure.
    pub async fn list_personal(
        &self,
        user_id: UserId,
        target: Target,
    ) -> Result<Vec<SavedView>, SavedViewError> {
        let sql = format!(
            "SELECT {SELECT_COLS} FROM saved_views \
             WHERE scope = 'personal' AND user_id = $1 AND target = $2 \
             ORDER BY created_at ASC"
        );
        let rows = sqlx::query(&sql)
            .bind(user_id.into_uuid())
            .bind(target.as_db_str())
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(row_to_view).collect()
    }

    /// Workspace-scope views for a target.
    ///
    /// # Errors
    ///
    /// [`SavedViewError::Db`] on backend failure.
    pub async fn list_workspace(&self, target: Target) -> Result<Vec<SavedView>, SavedViewError> {
        let sql = format!(
            "SELECT {SELECT_COLS} FROM saved_views \
             WHERE scope = 'workspace' AND target = $1 \
             ORDER BY created_at ASC"
        );
        let rows = sqlx::query(&sql)
            .bind(target.as_db_str())
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(row_to_view).collect()
    }

    // ── update / delete ─────────────────────────────────────

    /// Apply a partial update. Bumps `updated_at` on any
    /// touched field. Idempotent — all-None patch is a no-op.
    ///
    /// # Errors
    ///
    /// - [`SavedViewError::ViewNotFound`] if no row.
    /// - [`SavedViewError::InvalidInput`] if patch.name fails
    ///   validation.
    /// - [`SavedViewError::Db`].
    pub async fn update(&self, id: Uuid, patch: SavedViewPatch) -> Result<(), SavedViewError> {
        if let Some(name) = patch.name.as_deref() {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                return Err(SavedViewError::InvalidInput(
                    "name must not be empty".into(),
                ));
            }
            if trimmed.len() > MAX_NAME_LEN {
                return Err(SavedViewError::InvalidInput(format!(
                    "name too long: {} > {MAX_NAME_LEN}",
                    trimmed.len()
                )));
            }
        }
        let result = sqlx::query(
            r"
            UPDATE saved_views SET
                name       = COALESCE($2, name),
                payload    = COALESCE($3, payload),
                updated_at = now()
            WHERE id = $1
            ",
        )
        .bind(id)
        .bind(patch.name.as_deref())
        .bind(patch.payload.as_ref())
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(SavedViewError::ViewNotFound(id));
        }
        Ok(())
    }

    /// Delete. Idempotent (no error on missing row).
    ///
    /// # Errors
    ///
    /// [`SavedViewError::Db`] on backend failure.
    pub async fn delete(&self, id: Uuid) -> Result<(), SavedViewError> {
        sqlx::query("DELETE FROM saved_views WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

// ── helpers ──────────────────────────────────────────────────

fn validate_draft(d: &SavedViewDraft) -> Result<(), SavedViewError> {
    let trimmed = d.name.trim();
    if trimmed.is_empty() {
        return Err(SavedViewError::InvalidInput(
            "name must not be empty".into(),
        ));
    }
    if trimmed.len() > MAX_NAME_LEN {
        return Err(SavedViewError::InvalidInput(format!(
            "name too long: {} > {MAX_NAME_LEN}",
            trimmed.len()
        )));
    }
    // Scope-FK polarity.
    match d.scope {
        Scope::Personal => {
            if d.user_id.is_none() {
                return Err(SavedViewError::InvalidInput(
                    "Personal scope requires user_id".into(),
                ));
            }
        }
        Scope::Workspace => {
            if d.user_id.is_some() {
                return Err(SavedViewError::InvalidInput(
                    "Workspace scope must not set user_id".into(),
                ));
            }
        }
    }
    Ok(())
}

fn translate_fk(
    err: sqlx::Error,
    project_id: Option<ProjectId>,
    user_id: Option<UserId>,
    created_by: Option<UserId>,
) -> SavedViewError {
    if let sqlx::Error::Database(db_err) = &err
        && db_err.code().as_deref() == Some("23503")
    {
        let constraint = db_err.constraint().unwrap_or("");
        if constraint.contains("project")
            && let Some(p) = project_id
        {
            return SavedViewError::ProjectNotFound(p.into_uuid());
        }
        if let Some(u) = user_id {
            return SavedViewError::UserNotFound(u.into_uuid());
        }
        if let Some(u) = created_by {
            return SavedViewError::UserNotFound(u.into_uuid());
        }
        if let Some(p) = project_id {
            return SavedViewError::ProjectNotFound(p.into_uuid());
        }
    }
    SavedViewError::Db(err)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use sentori_workspace_identity::WorkspaceId;

    #[test]
    fn validate_rejects_empty_name() {
        let d = SavedViewDraft::new(WorkspaceId::new(), "  ", Target::Issues, Scope::Workspace);
        assert!(matches!(
            validate_draft(&d),
            Err(SavedViewError::InvalidInput(_))
        ));
    }

    #[test]
    fn validate_personal_requires_user() {
        let d = SavedViewDraft::new(WorkspaceId::new(), "x", Target::Issues, Scope::Personal);
        assert!(matches!(
            validate_draft(&d),
            Err(SavedViewError::InvalidInput(_))
        ));
    }

    #[test]
    fn validate_workspace_rejects_user() {
        let d = SavedViewDraft::new(WorkspaceId::new(), "x", Target::Issues, Scope::Workspace)
            .owned_by(UserId::new());
        assert!(matches!(
            validate_draft(&d),
            Err(SavedViewError::InvalidInput(_))
        ));
    }

    #[test]
    fn validate_accepts_personal_with_user() {
        let d = SavedViewDraft::new(WorkspaceId::new(), "x", Target::Issues, Scope::Personal)
            .owned_by(UserId::new());
        assert!(validate_draft(&d).is_ok());
    }

    #[test]
    fn validate_accepts_workspace_without_user() {
        let d = SavedViewDraft::new(WorkspaceId::new(), "x", Target::Issues, Scope::Workspace);
        assert!(validate_draft(&d).is_ok());
    }

    #[test]
    fn validate_rejects_oversize_name() {
        let long = "x".repeat(MAX_NAME_LEN + 1);
        let d = SavedViewDraft::new(WorkspaceId::new(), long, Target::Issues, Scope::Workspace);
        assert!(matches!(
            validate_draft(&d),
            Err(SavedViewError::InvalidInput(_))
        ));
    }
}
