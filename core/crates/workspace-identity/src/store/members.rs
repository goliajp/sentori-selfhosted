//! `workspace_members` table CRUD + owner-transfer transaction.

use sqlx::{PgPool, Row};
use time::OffsetDateTime;

use crate::WorkspaceId;
use crate::error::IdentityError;
use crate::model::{Member, Role, UserId};

/// Store sub-handle for `workspace_members`. Workspace-scoped.
#[derive(Debug, Clone, Copy)]
pub struct Members<'a> {
    pool: &'a PgPool,
    workspace_id: WorkspaceId,
}

impl<'a> Members<'a> {
    /// Construct over a borrowed pool + workspace id.
    #[must_use]
    pub const fn new(pool: &'a PgPool, workspace_id: WorkspaceId) -> Self {
        Self { pool, workspace_id }
    }

    /// Insert a new member.
    ///
    /// To insert an owner, the caller must first ensure no
    /// existing `role='owner'` row exists FOR THIS WORKSPACE —
    /// the DB-level partial unique index on (`workspace_id`) WHERE
    /// role='owner' will otherwise reject the insert.
    ///
    /// # Errors
    ///
    /// - [`IdentityError::Db`] on database failure (incl. the
    ///   FK to `users` failing, or the one-owner partial index).
    pub async fn add(
        &self,
        user_id: UserId,
        role: Role,
        added_by: Option<UserId>,
    ) -> Result<Member, IdentityError> {
        let row = sqlx::query(
            "INSERT INTO workspace_members (workspace_id, user_id, role, added_by) \
             VALUES ($1, $2, $3, $4) \
             RETURNING user_id, role, added_by, added_at",
        )
        .bind(self.workspace_id.into_uuid())
        .bind(user_id.into_uuid())
        .bind(role.as_db_str())
        .bind(added_by.map(UserId::into_uuid))
        .fetch_one(self.pool)
        .await?;

        row_to_member(&row)
    }

    /// Remove a member from this workspace.
    ///
    /// # Errors
    ///
    /// - [`IdentityError::NotAMember`] if no row exists for
    ///   `user_id` in this workspace.
    /// - [`IdentityError::Db`] on database failure.
    pub async fn remove(&self, user_id: UserId) -> Result<(), IdentityError> {
        let member = self
            .find(user_id)
            .await?
            .ok_or(IdentityError::NotAMember(user_id))?;
        if matches!(member.role, Role::Owner) {
            return Err(IdentityError::Db(sqlx::Error::Protocol(
                "cannot remove the sole owner — transfer first".into(),
            )));
        }
        let result =
            sqlx::query("DELETE FROM workspace_members WHERE workspace_id = $1 AND user_id = $2")
                .bind(self.workspace_id.into_uuid())
                .bind(user_id.into_uuid())
                .execute(self.pool)
                .await?;
        if result.rows_affected() == 0 {
            Err(IdentityError::NotAMember(user_id))
        } else {
            Ok(())
        }
    }

    /// Look up a member by id within this workspace.
    ///
    /// # Errors
    ///
    /// [`IdentityError::Db`] on database failure.
    pub async fn find(&self, user_id: UserId) -> Result<Option<Member>, IdentityError> {
        let row = sqlx::query(
            "SELECT user_id, role, added_by, added_at FROM workspace_members \
             WHERE workspace_id = $1 AND user_id = $2",
        )
        .bind(self.workspace_id.into_uuid())
        .bind(user_id.into_uuid())
        .fetch_optional(self.pool)
        .await?;

        row.as_ref().map(row_to_member).transpose()
    }

    /// List all members of this workspace, ordered by
    /// `added_at` ascending. Small workspaces; no pagination.
    ///
    /// # Errors
    ///
    /// [`IdentityError::Db`] on database failure.
    pub async fn list(&self) -> Result<Vec<Member>, IdentityError> {
        let rows = sqlx::query(
            "SELECT user_id, role, added_by, added_at FROM workspace_members \
             WHERE workspace_id = $1 ORDER BY added_at ASC",
        )
        .bind(self.workspace_id.into_uuid())
        .fetch_all(self.pool)
        .await?;

        rows.iter().map(row_to_member).collect()
    }

    /// Change a member's role within this workspace.
    ///
    /// # Errors
    ///
    /// - [`IdentityError::NotAMember`] if no row exists for
    ///   `user_id`.
    /// - [`IdentityError::Db`] for invariant violations and
    ///   underlying database errors.
    pub async fn set_role(&self, user_id: UserId, new_role: Role) -> Result<(), IdentityError> {
        if matches!(new_role, Role::Owner) {
            return Err(IdentityError::Db(sqlx::Error::Protocol(
                "use transfer_owner to set role=owner".into(),
            )));
        }

        let mut tx = self.pool.begin().await?;

        let current = sqlx::query(
            "SELECT role FROM workspace_members \
             WHERE workspace_id = $1 AND user_id = $2 FOR UPDATE",
        )
        .bind(self.workspace_id.into_uuid())
        .bind(user_id.into_uuid())
        .fetch_optional(&mut *tx)
        .await?;

        let current_role = current
            .map(|r| Role::from_db_str(r.get::<&str, _>("role")))
            .transpose()?
            .ok_or(IdentityError::NotAMember(user_id))?;

        if matches!(current_role, Role::Owner) {
            return Err(IdentityError::Db(sqlx::Error::Protocol(
                "cannot demote the sole owner — transfer first".into(),
            )));
        }

        sqlx::query(
            "UPDATE workspace_members SET role = $1 \
             WHERE workspace_id = $2 AND user_id = $3",
        )
        .bind(new_role.as_db_str())
        .bind(self.workspace_id.into_uuid())
        .bind(user_id.into_uuid())
        .execute(&mut *tx)
        .await?;

        if new_role.auto_sees_all_projects() {
            sqlx::query(
                "DELETE FROM project_user_visibility \
                 WHERE workspace_id = $1 AND user_id = $2",
            )
            .bind(self.workspace_id.into_uuid())
            .bind(user_id.into_uuid())
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    /// Transfer the owner role within this workspace.
    ///
    /// # Errors
    ///
    /// - [`IdentityError::TransferTargetNotMember`] if
    ///   `new_owner` has no `workspace_members` row in this
    ///   workspace.
    /// - [`IdentityError::TransferTargetAlreadyOwner`] if
    ///   `new_owner` is the current owner.
    /// - [`IdentityError::Db`] for underlying database errors.
    pub async fn transfer_owner(&self, new_owner: UserId) -> Result<(), IdentityError> {
        let mut tx = self.pool.begin().await?;

        let target = sqlx::query(
            "SELECT role FROM workspace_members \
             WHERE workspace_id = $1 AND user_id = $2 FOR UPDATE",
        )
        .bind(self.workspace_id.into_uuid())
        .bind(new_owner.into_uuid())
        .fetch_optional(&mut *tx)
        .await?;

        let target_role = target
            .map(|r| Role::from_db_str(r.get::<&str, _>("role")))
            .transpose()?
            .ok_or(IdentityError::TransferTargetNotMember(new_owner))?;

        if matches!(target_role, Role::Owner) {
            return Err(IdentityError::TransferTargetAlreadyOwner);
        }

        sqlx::query(
            "UPDATE workspace_members SET role = 'admin' \
             WHERE workspace_id = $1 AND role = 'owner'",
        )
        .bind(self.workspace_id.into_uuid())
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "UPDATE workspace_members SET role = 'owner' \
             WHERE workspace_id = $1 AND user_id = $2",
        )
        .bind(self.workspace_id.into_uuid())
        .bind(new_owner.into_uuid())
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "DELETE FROM project_user_visibility \
             WHERE workspace_id = $1 AND user_id = $2",
        )
        .bind(self.workspace_id.into_uuid())
        .bind(new_owner.into_uuid())
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    /// Find the owner of this workspace.
    ///
    /// # Errors
    ///
    /// [`IdentityError::Db`] on database failure.
    pub async fn find_owner(&self) -> Result<Option<Member>, IdentityError> {
        let row = sqlx::query(
            "SELECT user_id, role, added_by, added_at FROM workspace_members \
             WHERE workspace_id = $1 AND role = 'owner'",
        )
        .bind(self.workspace_id.into_uuid())
        .fetch_optional(self.pool)
        .await?;

        row.as_ref().map(row_to_member).transpose()
    }
}

fn row_to_member(row: &sqlx::postgres::PgRow) -> Result<Member, IdentityError> {
    Ok(Member {
        user_id: UserId::from_uuid(row.get("user_id")),
        role: Role::from_db_str(row.get::<&str, _>("role"))?,
        added_by: row
            .get::<Option<uuid::Uuid>, _>("added_by")
            .map(UserId::from_uuid),
        added_at: row.get::<OffsetDateTime, _>("added_at"),
    })
}
