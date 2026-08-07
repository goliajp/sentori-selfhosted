//! `email_verifications` CRUD.

use sentori_workspace_identity::{UserId, WorkspaceId};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::AuthError;
use crate::token::EmailVerifyToken;

/// `email_verifications` row (no token plaintext).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmailVerification {
    /// Primary key.
    pub id: Uuid,
    /// References [`sentori_workspace_identity::User::id`].
    pub user_id: UserId,
    /// Token expiration (after this, the token cannot be
    /// consumed).
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
    /// Set on accept. `None` for pending.
    #[serde(with = "time::serde::rfc3339::option")]
    pub used_at: Option<OffsetDateTime>,
    /// Creation timestamp.
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

impl EmailVerification {
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

/// Combined return value of [`EmailVerifications::create`].
#[derive(Debug)]
pub struct MintedEmailVerify {
    /// Persisted row (no plaintext token).
    pub verification: EmailVerification,
    /// Plaintext token — caller emails this to the recipient.
    pub plaintext_token: EmailVerifyToken,
}

/// Store sub-handle for `email_verifications`.
#[derive(Debug, Clone, Copy)]
pub struct EmailVerifications<'a> {
    pool: &'a PgPool,
    workspace_id: WorkspaceId,
}

impl<'a> EmailVerifications<'a> {
    /// Construct over a borrowed pool, scoped to a workspace
    /// (the table's `workspace_id` is NOT NULL).
    #[must_use]
    pub const fn new(pool: &'a PgPool, workspace_id: WorkspaceId) -> Self {
        Self { pool, workspace_id }
    }

    /// Mint a new verification token for `user_id` with the
    /// given absolute `expires_at`.
    ///
    /// # Errors
    ///
    /// - [`AuthError::Entropy`] on CSPRNG failure.
    /// - [`AuthError::Db`] on DB failure (incl. FK to users).
    pub async fn create(
        &self,
        user_id: UserId,
        expires_at: OffsetDateTime,
    ) -> Result<MintedEmailVerify, AuthError> {
        let token = EmailVerifyToken::generate()?;
        let id = Uuid::now_v7();

        let row = sqlx::query(
            "INSERT INTO email_verifications \
             (id, workspace_id, user_id, token_hash, expires_at) \
             VALUES ($1, $2, $3, $4, $5) \
             RETURNING id, user_id, expires_at, used_at, created_at",
        )
        .bind(id)
        .bind(self.workspace_id.into_uuid())
        .bind(user_id.into_uuid())
        .bind(token.hash().as_bytes().as_slice())
        .bind(expires_at)
        .fetch_one(self.pool)
        .await?;

        let verification = row_to_verification(&row);
        Ok(MintedEmailVerify {
            verification,
            plaintext_token: token,
        })
    }

    /// Consume a verification token. On success, marks the
    /// row's `used_at` and returns the owning `user_id` — caller
    /// then flips `users.email_verified` (typically inside a
    /// transaction).
    ///
    /// Use this as part of [`crate::AuthService::verify_email`]
    /// (which wraps both writes in one transaction). Bare calls
    /// to `consume` outside that flow are valid for tests but
    /// leave the user un-verified.
    ///
    /// # Errors
    ///
    /// - [`AuthError::TokenInvalid`] for bad format, no
    ///   matching row, expired, or already-used. Collapsed
    ///   deliberately so attackers can't enumerate states.
    /// - [`AuthError::Db`] on DB failure.
    pub async fn consume(&self, token_wire: &str) -> Result<UserId, AuthError> {
        let hash = EmailVerifyToken::parse_and_hash(token_wire)?;
        let mut tx = self.pool.begin().await?;

        // SELECT FOR UPDATE serializes concurrent accept attempts.
        let row = sqlx::query(
            "SELECT id, user_id, expires_at, used_at, created_at \
             FROM email_verifications \
             WHERE token_hash = $1 FOR UPDATE",
        )
        .bind(hash.as_bytes().as_slice())
        .fetch_optional(&mut *tx)
        .await?;

        let row = row.ok_or(AuthError::TokenInvalid)?;
        let verification = row_to_verification(&row);
        let now = OffsetDateTime::now_utc();
        if !verification.is_pending() || verification.is_expired(now) {
            return Err(AuthError::TokenInvalid);
        }

        let updated = sqlx::query(
            "UPDATE email_verifications SET used_at = $1 \
             WHERE id = $2 AND used_at IS NULL",
        )
        .bind(now)
        .bind(verification.id)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() == 0 {
            return Err(AuthError::TokenInvalid);
        }

        tx.commit().await?;
        Ok(verification.user_id)
    }

    /// Delete every verification (pending + used) for a user.
    /// Used when a user successfully verifies via an old token
    /// so stale ones can't accumulate.
    ///
    /// # Errors
    ///
    /// [`AuthError::Db`] on DB failure.
    pub async fn purge_for_user(&self, user_id: UserId) -> Result<u64, AuthError> {
        let result = sqlx::query("DELETE FROM email_verifications WHERE user_id = $1")
            .bind(user_id.into_uuid())
            .execute(self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// List pending (not-used + not-expired) verifications for
    /// a user — used by the dashboard's "resend" UI to show
    /// whether there's already one in flight.
    ///
    /// # Errors
    ///
    /// [`AuthError::Db`] on DB failure.
    pub async fn list_pending_for_user(
        &self,
        user_id: UserId,
    ) -> Result<Vec<EmailVerification>, AuthError> {
        let rows = sqlx::query(
            "SELECT id, user_id, expires_at, used_at, created_at \
             FROM email_verifications \
             WHERE user_id = $1 AND used_at IS NULL AND expires_at > now() \
             ORDER BY created_at DESC",
        )
        .bind(user_id.into_uuid())
        .fetch_all(self.pool)
        .await?;
        Ok(rows.iter().map(row_to_verification).collect())
    }
}

fn row_to_verification(row: &sqlx::postgres::PgRow) -> EmailVerification {
    EmailVerification {
        id: row.get::<Uuid, _>("id"),
        user_id: UserId::from_uuid(row.get("user_id")),
        expires_at: row.get::<OffsetDateTime, _>("expires_at"),
        used_at: row.get::<Option<OffsetDateTime>, _>("used_at"),
        created_at: row.get::<OffsetDateTime, _>("created_at"),
    }
}
