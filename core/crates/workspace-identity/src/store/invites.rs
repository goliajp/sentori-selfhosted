//! `workspace_invites` table CRUD + accept flow.

use sqlx::{PgPool, Row};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::WorkspaceId;
use crate::error::IdentityError;
use crate::invite_token::{InviteToken, MintedInvite};
use crate::model::{InviteRole, Member, UserId, WorkspaceInvite};

/// Store sub-handle for `workspace_invites`. Workspace-scoped.
#[derive(Debug, Clone, Copy)]
pub struct Invites<'a> {
    pool: &'a PgPool,
    workspace_id: WorkspaceId,
}

impl<'a> Invites<'a> {
    /// Construct over a borrowed pool + workspace id.
    #[must_use]
    pub const fn new(pool: &'a PgPool, workspace_id: WorkspaceId) -> Self {
        Self { pool, workspace_id }
    }

    /// Maximum allowed `expires_in_days` argument to
    /// [`Self::create`].
    pub const MAX_EXPIRES_IN_DAYS: i64 = 30;

    /// Mint a new invite in this workspace.
    ///
    /// # Errors
    ///
    /// - [`IdentityError::InviteExpiryOutOfRange`] if
    ///   `expires_in_days <= 0` or `> MAX_EXPIRES_IN_DAYS`.
    /// - [`IdentityError::Entropy`] if the CSPRNG fails.
    /// - [`IdentityError::Db`] on database failure.
    pub async fn create(
        &self,
        email: &str,
        role: InviteRole,
        invited_by: UserId,
        expires_in_days: i64,
    ) -> Result<MintedInvite, IdentityError> {
        if expires_in_days <= 0 || expires_in_days > Self::MAX_EXPIRES_IN_DAYS {
            return Err(IdentityError::InviteExpiryOutOfRange {
                got: expires_in_days,
                max: Self::MAX_EXPIRES_IN_DAYS,
            });
        }

        let token = InviteToken::generate()?;
        let token_hash = token.hash();
        let id = Uuid::now_v7();
        let expires_at = OffsetDateTime::now_utc() + Duration::days(expires_in_days);

        let row = sqlx::query(
            "INSERT INTO workspace_invites \
             (id, workspace_id, email, role, invited_by, token_hash, expires_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) \
             RETURNING id, email, role, invited_by, expires_at, accepted_at, created_at",
        )
        .bind(id)
        .bind(self.workspace_id.into_uuid())
        .bind(email)
        .bind(role.as_db_str())
        .bind(invited_by.into_uuid())
        .bind(token_hash.as_bytes().as_slice())
        .bind(expires_at)
        .fetch_one(self.pool)
        .await?;

        let invite = row_to_invite(&row)?;
        Ok(MintedInvite {
            invite,
            plaintext_token: token,
        })
    }

    /// List pending invites in this workspace.
    ///
    /// # Errors
    ///
    /// [`IdentityError::Db`] on database failure.
    pub async fn list_pending(&self) -> Result<Vec<WorkspaceInvite>, IdentityError> {
        let rows = sqlx::query(
            "SELECT id, email, role, invited_by, expires_at, accepted_at, created_at \
             FROM workspace_invites \
             WHERE workspace_id = $1 AND accepted_at IS NULL AND expires_at > now() \
             ORDER BY created_at ASC",
        )
        .bind(self.workspace_id.into_uuid())
        .fetch_all(self.pool)
        .await?;
        rows.iter().map(row_to_invite).collect()
    }

    /// List every invite in this workspace.
    ///
    /// # Errors
    ///
    /// [`IdentityError::Db`] on database failure.
    pub async fn list_all(&self) -> Result<Vec<WorkspaceInvite>, IdentityError> {
        let rows = sqlx::query(
            "SELECT id, email, role, invited_by, expires_at, accepted_at, created_at \
             FROM workspace_invites \
             WHERE workspace_id = $1 \
             ORDER BY created_at ASC",
        )
        .bind(self.workspace_id.into_uuid())
        .fetch_all(self.pool)
        .await?;
        rows.iter().map(row_to_invite).collect()
    }

    /// Revoke a pending invite.
    ///
    /// # Errors
    ///
    /// [`IdentityError::Db`] on database failure.
    pub async fn revoke(&self, id: Uuid) -> Result<(), IdentityError> {
        sqlx::query(
            "DELETE FROM workspace_invites \
             WHERE workspace_id = $1 AND id = $2 AND accepted_at IS NULL",
        )
        .bind(self.workspace_id.into_uuid())
        .bind(id)
        .execute(self.pool)
        .await?;
        Ok(())
    }

    /// Accept an invite. Validates token + workspace + freshness,
    /// marks invite accepted, inserts matching `workspace_members`
    /// row — all in one transaction.
    ///
    /// # Errors
    ///
    /// - [`IdentityError::InviteInvalid`] for bad token / no
    ///   match / wrong workspace / expired / already-accepted.
    /// - [`IdentityError::Db`] on database failure.
    pub async fn accept(&self, token_wire: &str, user_id: UserId) -> Result<Member, IdentityError> {
        let token_hash = InviteToken::parse_and_hash(token_wire)?;

        let mut tx = self.pool.begin().await?;

        let row = sqlx::query(
            "SELECT id, email, role, invited_by, expires_at, accepted_at, created_at \
             FROM workspace_invites \
             WHERE workspace_id = $1 AND token_hash = $2 FOR UPDATE",
        )
        .bind(self.workspace_id.into_uuid())
        .bind(token_hash.as_bytes().as_slice())
        .fetch_optional(&mut *tx)
        .await?;

        let row = row.ok_or(IdentityError::InviteInvalid)?;
        let invite = row_to_invite(&row)?;

        let now = OffsetDateTime::now_utc();
        if !invite.is_pending() || invite.is_expired(now) {
            return Err(IdentityError::InviteInvalid);
        }

        let updated = sqlx::query(
            "UPDATE workspace_invites SET accepted_at = $1 \
             WHERE workspace_id = $2 AND id = $3 AND accepted_at IS NULL",
        )
        .bind(now)
        .bind(self.workspace_id.into_uuid())
        .bind(invite.id)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() == 0 {
            return Err(IdentityError::InviteInvalid);
        }

        let role = invite.role.to_role();
        let member_row = sqlx::query(
            "INSERT INTO workspace_members (workspace_id, user_id, role, added_by) \
             VALUES ($1, $2, $3, $4) \
             RETURNING user_id, role, added_by, added_at",
        )
        .bind(self.workspace_id.into_uuid())
        .bind(user_id.into_uuid())
        .bind(role.as_db_str())
        .bind(invite.invited_by.into_uuid())
        .fetch_one(&mut *tx)
        .await?;

        let member = row_to_member(&member_row)?;

        tx.commit().await?;
        Ok(member)
    }
}

fn row_to_invite(row: &sqlx::postgres::PgRow) -> Result<WorkspaceInvite, IdentityError> {
    Ok(WorkspaceInvite {
        id: row.get::<Uuid, _>("id"),
        email: row.get("email"),
        role: InviteRole::from_db_str(row.get::<&str, _>("role")).map_err(|e| {
            IdentityError::InvalidRoleInDatabase(crate::model::RoleParseError(format!(
                "invite-role: {e}"
            )))
        })?,
        invited_by: UserId::from_uuid(row.get::<Uuid, _>("invited_by")),
        expires_at: row.get::<OffsetDateTime, _>("expires_at"),
        accepted_at: row.get::<Option<OffsetDateTime>, _>("accepted_at"),
        created_at: row.get::<OffsetDateTime, _>("created_at"),
    })
}

fn row_to_member(row: &sqlx::postgres::PgRow) -> Result<Member, IdentityError> {
    Ok(Member {
        user_id: UserId::from_uuid(row.get("user_id")),
        role: crate::model::Role::from_db_str(row.get::<&str, _>("role"))?,
        added_by: row
            .get::<Option<Uuid>, _>("added_by")
            .map(UserId::from_uuid),
        added_at: row.get::<OffsetDateTime, _>("added_at"),
    })
}
