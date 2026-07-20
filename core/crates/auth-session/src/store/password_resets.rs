//! `password_resets` CRUD.

use sentori_workspace_identity::UserId;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::AuthError;
use crate::token::PasswordResetToken;

/// `password_resets` row (no token plaintext).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PasswordReset {
    /// Primary key.
    pub id: Uuid,
    /// References [`sentori_workspace_identity::User::id`].
    pub user_id: UserId,
    /// Token expiration.
    pub expires_at: OffsetDateTime,
    /// Set on accept. `None` for pending.
    pub used_at: Option<OffsetDateTime>,
    /// Creation timestamp.
    pub created_at: OffsetDateTime,
}

impl PasswordReset {
    /// True if not yet consumed.
    #[must_use]
    pub const fn is_pending(&self) -> bool {
        self.used_at.is_none()
    }

    /// True if `now` is past `expires_at`.
    #[must_use]
    pub fn is_expired(&self, now: OffsetDateTime) -> bool {
        self.expires_at <= now
    }
}

/// Combined return value of [`PasswordResets::create`].
#[derive(Debug)]
pub struct MintedPasswordReset {
    /// Persisted row (no plaintext).
    pub reset: PasswordReset,
    /// Plaintext token — caller emails this to the recipient.
    pub plaintext_token: PasswordResetToken,
}

/// Store sub-handle for `password_resets`.
#[derive(Debug, Clone, Copy)]
pub struct PasswordResets<'a> {
    pool: &'a PgPool,
}

impl<'a> PasswordResets<'a> {
    /// Construct over a borrowed pool.
    #[must_use]
    pub const fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// Mint a new password-reset token for `user_id`.
    ///
    /// # Errors
    ///
    /// - [`AuthError::Entropy`] on CSPRNG failure.
    /// - [`AuthError::Db`] on DB failure (incl. FK to users).
    pub async fn create(
        &self,
        user_id: UserId,
        expires_at: OffsetDateTime,
    ) -> Result<MintedPasswordReset, AuthError> {
        let token = PasswordResetToken::generate()?;
        let id = Uuid::now_v7();

        let row = sqlx::query(
            "INSERT INTO password_resets \
             (id, user_id, token_hash, expires_at) \
             VALUES ($1, $2, $3, $4) \
             RETURNING id, user_id, expires_at, used_at, created_at",
        )
        .bind(id)
        .bind(user_id.into_uuid())
        .bind(token.hash().as_bytes().as_slice())
        .bind(expires_at)
        .fetch_one(self.pool)
        .await?;

        let reset = row_to_reset(&row);
        Ok(MintedPasswordReset {
            reset,
            plaintext_token: token,
        })
    }

    /// Consume a reset token. Marks `used_at` and returns the
    /// owning `user_id`. The caller rotates the password and
    /// invalidates sessions in the same transaction —
    /// [`crate::AuthService::reset_password`] does both.
    ///
    /// # Errors
    ///
    /// - [`AuthError::TokenInvalid`] for bad format, no
    ///   matching row, expired, or already used. Collapsed.
    /// - [`AuthError::Db`] on DB failure.
    pub async fn consume(&self, token_wire: &str) -> Result<UserId, AuthError> {
        let hash = PasswordResetToken::parse_and_hash(token_wire)?;
        let mut tx = self.pool.begin().await?;

        let row = sqlx::query(
            "SELECT id, user_id, expires_at, used_at, created_at \
             FROM password_resets \
             WHERE token_hash = $1 FOR UPDATE",
        )
        .bind(hash.as_bytes().as_slice())
        .fetch_optional(&mut *tx)
        .await?;

        let row = row.ok_or(AuthError::TokenInvalid)?;
        let reset = row_to_reset(&row);
        let now = OffsetDateTime::now_utc();
        if !reset.is_pending() || reset.is_expired(now) {
            return Err(AuthError::TokenInvalid);
        }

        let updated = sqlx::query(
            "UPDATE password_resets SET used_at = $1 \
             WHERE id = $2 AND used_at IS NULL",
        )
        .bind(now)
        .bind(reset.id)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() == 0 {
            return Err(AuthError::TokenInvalid);
        }

        tx.commit().await?;
        Ok(reset.user_id)
    }

    /// Delete every reset row (pending + used) for a user.
    /// Defensive — keeps the table tidy after a successful
    /// reset.
    ///
    /// # Errors
    ///
    /// [`AuthError::Db`] on DB failure.
    pub async fn purge_for_user(&self, user_id: UserId) -> Result<u64, AuthError> {
        let result = sqlx::query("DELETE FROM password_resets WHERE user_id = $1")
            .bind(user_id.into_uuid())
            .execute(self.pool)
            .await?;
        Ok(result.rows_affected())
    }
}

fn row_to_reset(row: &sqlx::postgres::PgRow) -> PasswordReset {
    PasswordReset {
        id: row.get::<Uuid, _>("id"),
        user_id: UserId::from_uuid(row.get("user_id")),
        expires_at: row.get::<OffsetDateTime, _>("expires_at"),
        used_at: row.get::<Option<OffsetDateTime>, _>("used_at"),
        created_at: row.get::<OffsetDateTime, _>("created_at"),
    }
}
