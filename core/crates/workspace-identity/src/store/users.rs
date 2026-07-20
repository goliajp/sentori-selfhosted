//! `users` table CRUD.
//!
//! [`Users`] is workspace-scoped for `create()` (which writes
//! `workspace_id NOT NULL`). All other methods are keyed by
//! user-id PK (globally unique), so they don't carry a workspace
//! filter at the application layer — production deployments rely
//! on Postgres RLS to enforce per-workspace isolation against a
//! non-owner login role. The dev / test path runs as superuser
//! and bypasses RLS naturally.

use sqlx::{PgPool, Row};
use time::OffsetDateTime;

use crate::WorkspaceId;
use crate::error::IdentityError;
use crate::model::{User, UserId};

/// Store sub-handle for the `users` table. Holds a workspace
/// scope for `create()`; other methods are PK-keyed and don't
/// depend on the scope.
#[derive(Debug, Clone, Copy)]
pub struct Users<'a> {
    pool: &'a PgPool,
    workspace_id: WorkspaceId,
}

impl<'a> Users<'a> {
    /// Construct over a borrowed pool. Typically obtained via
    /// [`crate::Identity::users`].
    #[must_use]
    pub const fn new(pool: &'a PgPool, workspace_id: WorkspaceId) -> Self {
        Self { pool, workspace_id }
    }

    /// Create a new user in this workspace.
    ///
    /// # Errors
    ///
    /// - [`IdentityError::EmailTaken`] if a row with the same
    ///   `LOWER(email)` already exists (emails are globally
    ///   unique across workspaces).
    /// - [`IdentityError::Db`] for any other database error.
    pub async fn create(&self, email: &str, password_hash: &str) -> Result<User, IdentityError> {
        let id = UserId::new();
        let row = sqlx::query(
            "INSERT INTO users (id, workspace_id, email, password_hash, email_verified) \
             VALUES ($1, $2, $3, $4, FALSE) \
             RETURNING id, email, password_hash, email_verified, created_at",
        )
        .bind(id.into_uuid())
        .bind(self.workspace_id.into_uuid())
        .bind(email)
        .bind(password_hash)
        .fetch_one(self.pool)
        .await
        .map_err(translate_unique_email)?;

        Ok(row_to_user(&row))
    }

    /// Find a user by id (PK — global lookup; RLS protects in
    /// production).
    ///
    /// # Errors
    ///
    /// [`IdentityError::Db`] on database failure.
    pub async fn find_by_id(&self, id: UserId) -> Result<Option<User>, IdentityError> {
        let row = sqlx::query(
            "SELECT id, email, password_hash, email_verified, created_at \
             FROM users WHERE id = $1",
        )
        .bind(id.into_uuid())
        .fetch_optional(self.pool)
        .await?;

        Ok(row.as_ref().map(row_to_user))
    }

    /// Find a user by email (case-insensitive). Cross-workspace
    /// — emails are globally unique, used by the login flow
    /// before workspace context is known.
    ///
    /// # Errors
    ///
    /// [`IdentityError::Db`] on database failure.
    pub async fn find_by_email(&self, email: &str) -> Result<Option<User>, IdentityError> {
        let row = sqlx::query(
            "SELECT id, email, password_hash, email_verified, created_at \
             FROM users WHERE LOWER(email) = LOWER($1)",
        )
        .bind(email)
        .fetch_optional(self.pool)
        .await?;

        Ok(row.as_ref().map(row_to_user))
    }

    /// Return the stored `password_hash` for an account, or
    /// `None` if no such account exists. Cross-workspace login
    /// helper.
    ///
    /// # Errors
    ///
    /// [`IdentityError::Db`] on database failure.
    pub async fn lookup_password_hash(
        &self,
        email: &str,
    ) -> Result<Option<(UserId, String)>, IdentityError> {
        let row = sqlx::query("SELECT id, password_hash FROM users WHERE LOWER(email) = LOWER($1)")
            .bind(email)
            .fetch_optional(self.pool)
            .await?;

        Ok(row.map(|r| {
            (
                UserId::from_uuid(r.get::<uuid::Uuid, _>("id")),
                r.get::<String, _>("password_hash"),
            )
        }))
    }

    /// Resolve the workspace a user belongs to (for login flow
    /// — caller uses the returned `WorkspaceId` to construct a
    /// scoped Identity for subsequent operations).
    ///
    /// # Errors
    ///
    /// [`IdentityError::Db`] on database failure.
    pub async fn resolve_workspace(
        &self,
        id: UserId,
    ) -> Result<Option<WorkspaceId>, IdentityError> {
        let row = sqlx::query("SELECT workspace_id FROM users WHERE id = $1")
            .bind(id.into_uuid())
            .fetch_optional(self.pool)
            .await?;
        Ok(row.map(|r| WorkspaceId::from_uuid(r.get::<uuid::Uuid, _>("workspace_id"))))
    }

    /// Flip `email_verified` to true. Keyed by user-id PK
    /// (cross-workspace); RLS protects in production.
    ///
    /// # Errors
    ///
    /// - [`IdentityError::UserNotFound`] if no matching row.
    /// - [`IdentityError::Db`] on database failure.
    pub async fn mark_email_verified(&self, id: UserId) -> Result<(), IdentityError> {
        let result = sqlx::query("UPDATE users SET email_verified = TRUE WHERE id = $1")
            .bind(id.into_uuid())
            .execute(self.pool)
            .await?;

        if result.rows_affected() == 0 {
            Err(IdentityError::UserNotFound(id))
        } else {
            Ok(())
        }
    }

    /// Update the stored password hash. Keyed by user-id PK
    /// (cross-workspace); RLS protects in production.
    ///
    /// # Errors
    ///
    /// - [`IdentityError::UserNotFound`] if no matching row.
    /// - [`IdentityError::Db`] on database failure.
    pub async fn update_password_hash(
        &self,
        id: UserId,
        new_hash: &str,
    ) -> Result<(), IdentityError> {
        let result = sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
            .bind(new_hash)
            .bind(id.into_uuid())
            .execute(self.pool)
            .await?;

        if result.rows_affected() == 0 {
            Err(IdentityError::UserNotFound(id))
        } else {
            Ok(())
        }
    }
}

fn row_to_user(row: &sqlx::postgres::PgRow) -> User {
    User {
        id: UserId::from_uuid(row.get("id")),
        email: row.get("email"),
        email_verified: row.get("email_verified"),
        created_at: row.get::<OffsetDateTime, _>("created_at"),
    }
}

fn translate_unique_email(err: sqlx::Error) -> IdentityError {
    if let sqlx::Error::Database(db_err) = &err
        && db_err.code().as_deref() == Some("23505")
        && db_err
            .constraint()
            .is_none_or(|c| c == "users_email_ci_idx" || c == "users_pkey")
    {
        return IdentityError::EmailTaken;
    }
    IdentityError::Db(err)
}
