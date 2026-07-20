//! `push_tokens` CRUD: register / lookup / invalidate / quarantine.

use sentori_workspace_identity::ProjectId;
use sqlx::{PgPool, Row};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::PushError;
use crate::model::{DeviceToken, MintedToken, ProviderKind};

/// Store sub-handle for `push_tokens`.
#[derive(Debug, Clone)]
pub struct DeviceTokenStore {
    pool: PgPool,
}

impl DeviceTokenStore {
    /// Construct.
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Register a device token, or bump `last_seen_at` (and
    /// `app_user_id` if supplied) on the existing row.
    ///
    /// Idempotent on `(project_id, kind, native_token)`.
    ///
    /// # Errors
    ///
    /// - [`PushError::ProjectNotFound`] on FK violation.
    /// - [`PushError::InvalidInput`] for empty `native_token`.
    /// - [`PushError::Db`] on database failure.
    pub async fn upsert(
        &self,
        project_id: ProjectId,
        kind: ProviderKind,
        native_token: &str,
        env: Option<&str>,
        app_user_id: Option<&str>,
    ) -> Result<MintedToken, PushError> {
        if native_token.trim().is_empty() {
            return Err(PushError::InvalidInput(
                "native_token must not be empty".into(),
            ));
        }
        let new_id = Uuid::now_v7();
        let row = sqlx::query(
            r"
            INSERT INTO push_tokens
                (id, workspace_id, project_id, kind, native_token, env, app_user_id)
            SELECT $1, p.workspace_id, $2, $3, $4, $5, $6
            FROM projects p WHERE p.id = $2
            ON CONFLICT (project_id, kind, native_token) DO UPDATE SET
                last_seen_at = now(),
                env          = COALESCE(EXCLUDED.env, push_tokens.env),
                app_user_id  = COALESCE(EXCLUDED.app_user_id, push_tokens.app_user_id),
                -- Re-registering a previously-quarantined token
                -- means the device is alive again; clear the
                -- quarantine so dispatch will retry. The next
                -- failing send re-quarantines.
                quarantined_at   = NULL,
                quarantine_reason = NULL
            RETURNING id, (xmax = 0) AS is_new
            ",
        )
        .bind(new_id)
        .bind(project_id.into_uuid())
        .bind(kind.as_db_str())
        .bind(native_token)
        .bind(env)
        .bind(app_user_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| translate_fk(e, project_id))?;

        Ok(MintedToken {
            id: row.get("id"),
            is_new: row.get("is_new"),
        })
    }

    /// Look up a single token by id.
    ///
    /// # Errors
    ///
    /// [`PushError::Db`] on database failure.
    pub async fn find(&self, id: Uuid) -> Result<Option<DeviceToken>, PushError> {
        let row = sqlx::query(
            r"
            SELECT id, project_id, kind, native_token, env, app_user_id,
                   created_at, last_seen_at, quarantined_at, quarantine_reason
            FROM push_tokens WHERE id = $1
            ",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(row_to_token).transpose()
    }

    /// List all live (non-quarantined) tokens for
    /// `(project, kind)`. Used by the dispatcher when target
    /// is `ProjectKind`.
    ///
    /// # Errors
    ///
    /// [`PushError::Db`] on database failure.
    pub async fn list_live(
        &self,
        project_id: ProjectId,
        kind: ProviderKind,
    ) -> Result<Vec<DeviceToken>, PushError> {
        let rows = sqlx::query(
            r"
            SELECT id, project_id, kind, native_token, env, app_user_id,
                   created_at, last_seen_at, quarantined_at, quarantine_reason
            FROM push_tokens
            WHERE project_id = $1 AND kind = $2 AND quarantined_at IS NULL
            ORDER BY last_seen_at DESC
            ",
        )
        .bind(project_id.into_uuid())
        .bind(kind.as_db_str())
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_token).collect()
    }

    /// List live tokens for a specific app-side user across
    /// every provider.
    ///
    /// # Errors
    ///
    /// [`PushError::Db`] on database failure.
    pub async fn list_for_user(
        &self,
        project_id: ProjectId,
        app_user_id: &str,
    ) -> Result<Vec<DeviceToken>, PushError> {
        let rows = sqlx::query(
            r"
            SELECT id, project_id, kind, native_token, env, app_user_id,
                   created_at, last_seen_at, quarantined_at, quarantine_reason
            FROM push_tokens
            WHERE project_id = $1 AND app_user_id = $2 AND quarantined_at IS NULL
            ORDER BY last_seen_at DESC
            ",
        )
        .bind(project_id.into_uuid())
        .bind(app_user_id)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_token).collect()
    }

    /// Stamp `quarantined_at = now()` + record the reason.
    /// Idempotent — repeated calls just refresh the
    /// reason / timestamp.
    ///
    /// # Errors
    ///
    /// [`PushError::Db`] on database failure.
    pub async fn quarantine(&self, id: Uuid, reason: &str) -> Result<(), PushError> {
        sqlx::query(
            r"
            UPDATE push_tokens
            SET quarantined_at = COALESCE(quarantined_at, now()),
                quarantine_reason = $1
            WHERE id = $2
            ",
        )
        .bind(reason)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Hard-delete (operator's "Forget this device"). Returns
    /// `Ok(())` whether or not the row existed.
    ///
    /// # Errors
    ///
    /// [`PushError::Db`] on database failure.
    pub async fn delete(&self, id: Uuid) -> Result<(), PushError> {
        sqlx::query("DELETE FROM push_tokens WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

fn row_to_token(row: &sqlx::postgres::PgRow) -> Result<DeviceToken, PushError> {
    let kind_str: &str = row.get("kind");
    Ok(DeviceToken {
        id: row.get("id"),
        project_id: ProjectId::from_uuid(row.get("project_id")),
        kind: ProviderKind::from_db_str(kind_str)?,
        native_token: row.get("native_token"),
        env: row.get::<Option<String>, _>("env"),
        app_user_id: row.get::<Option<String>, _>("app_user_id"),
        created_at: row.get::<OffsetDateTime, _>("created_at"),
        last_seen_at: row.get::<OffsetDateTime, _>("last_seen_at"),
        quarantined_at: row.get::<Option<OffsetDateTime>, _>("quarantined_at"),
        quarantine_reason: row.get::<Option<String>, _>("quarantine_reason"),
    })
}

fn translate_fk(err: sqlx::Error, project_id: ProjectId) -> PushError {
    if let sqlx::Error::Database(db_err) = &err
        && db_err.code().as_deref() == Some("23503")
    {
        return PushError::ProjectNotFound(project_id.into_uuid());
    }
    PushError::Db(err)
}
