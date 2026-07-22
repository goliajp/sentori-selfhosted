//! `push_credentials` CRUD + S12 seal/unseal.

use std::sync::Arc;

use sentori_secrets_vault::Vault;
use sentori_workspace_identity::ProjectId;
use serde_json::Value;
use sqlx::{PgPool, Row};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::PushError;
use crate::model::{ProviderKind, ValidateOutcome};

/// `push_credentials` row in cleartext, with the secret bytes
/// already unsealed via [`sentori_secrets_vault::Vault::open`].
#[derive(Debug, Clone)]
pub struct StoredCredential {
    /// Primary key.
    pub id: Uuid,
    /// Owning project.
    pub project_id: ProjectId,
    /// Which provider this credential is for.
    pub kind: ProviderKind,
    /// Non-secret config (vendor-shape JSONB).
    pub config: Value,
    /// Decrypted secret bytes (vault-unsealed).
    pub secret_payload: Vec<u8>,
    /// Creation timestamp.
    pub created_at: OffsetDateTime,
    /// Last `validate()` time.
    pub last_validated_at: Option<OffsetDateTime>,
    /// Last `validate()` stable status string.
    pub last_validate_status: Option<String>,
}

/// Store sub-handle for `push_credentials`.
///
/// Holds the S12 `Vault` behind an `Arc` so the
/// [`CredentialStore`] (and [`crate::PushDispatcher`] that
/// owns it) stays `Clone` — needed for the consumer crate
/// to share the dispatcher across handler tasks.
#[derive(Debug, Clone)]
pub struct CredentialStore {
    pool: PgPool,
    vault: Arc<Vault>,
}

impl CredentialStore {
    /// Construct from an owned [`Vault`]. The vault is wrapped
    /// in an `Arc` internally; pass it by value here.
    #[must_use]
    pub fn new(pool: PgPool, vault: Vault) -> Self {
        Self {
            pool,
            vault: Arc::new(vault),
        }
    }

    /// Construct from a pre-shared [`Arc<Vault>`] — convenient
    /// when the consumer crate already holds the vault behind
    /// an `Arc` (e.g. for cross-crate sharing).
    #[must_use]
    pub const fn from_arc(pool: PgPool, vault: Arc<Vault>) -> Self {
        Self { pool, vault }
    }

    /// UPSERT a credential row. Sealing happens here — caller
    /// passes plaintext `secret_payload` bytes (APNs .p8 PEM,
    /// FCM service-account JSON, etc.) and we ship to vault
    /// then to DB.
    ///
    /// On conflict `(project_id, kind)` updates `config` +
    /// `secret_blob` (no longer auto-validates — caller
    /// invokes `validate` separately for green/red UI).
    ///
    /// # Errors
    ///
    /// - [`PushError::ProjectNotFound`] on FK violation.
    /// - [`PushError::CredentialUnseal`] (via `SealError`) is
    ///   NOT possible here (we don't open); seal failure is
    ///   wrapped as [`PushError::InvalidInput`].
    /// - [`PushError::Db`] on database failure.
    pub async fn upsert(
        &self,
        project_id: ProjectId,
        kind: ProviderKind,
        config: &Value,
        secret_payload: &[u8],
    ) -> Result<Uuid, PushError> {
        let sealed = self
            .vault
            .seal(secret_payload)
            .map_err(|e| PushError::InvalidInput(format!("credential seal failed: {e}")))?;
        let new_id = Uuid::now_v7();
        let row = sqlx::query(
            r"
            INSERT INTO push_credentials
                (id, workspace_id, project_id, kind, config, secret_blob)
            SELECT $1, p.workspace_id, $2, $3, $4, $5
            FROM projects p WHERE p.id = $2
            ON CONFLICT (project_id, kind) DO UPDATE SET
                config = EXCLUDED.config,
                secret_blob = EXCLUDED.secret_blob,
                last_validated_at = NULL,
                last_validate_status = NULL
            RETURNING id
            ",
        )
        .bind(new_id)
        .bind(project_id.into_uuid())
        .bind(kind.as_db_str())
        .bind(config)
        .bind(sealed.as_slice())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| translate_fk(e, project_id))?;
        // Unknown project → the driving SELECT matches zero rows → nothing is
        // inserted, the ON CONFLICT branch never runs and no FK violation is
        // raised. Absence of a RETURNING row is the only signal.
        let row = row.ok_or_else(|| PushError::ProjectNotFound(project_id.into_uuid()))?;
        Ok(row.get::<Uuid, _>("id"))
    }

    /// Load + unseal a credential for `(project, kind)`.
    /// Returns `None` if no row.
    ///
    /// # Errors
    ///
    /// - [`PushError::CredentialUnseal`] if the seal is
    ///   corrupt or sealed with a different master key.
    /// - [`PushError::Db`] on database failure.
    pub async fn load(
        &self,
        project_id: ProjectId,
        kind: ProviderKind,
    ) -> Result<Option<StoredCredential>, PushError> {
        let row = sqlx::query(
            r"
            SELECT id, project_id, kind, config, secret_blob, created_at,
                   last_validated_at, last_validate_status
            FROM push_credentials
            WHERE project_id = $1 AND kind = $2
            ",
        )
        .bind(project_id.into_uuid())
        .bind(kind.as_db_str())
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else { return Ok(None) };
        let sealed: Vec<u8> = row.get("secret_blob");
        let secret_payload = self.vault.open(&sealed)?;
        Ok(Some(StoredCredential {
            id: row.get("id"),
            project_id: ProjectId::from_uuid(row.get("project_id")),
            kind,
            config: row.get("config"),
            secret_payload,
            created_at: row.get("created_at"),
            last_validated_at: row.get("last_validated_at"),
            last_validate_status: row.get("last_validate_status"),
        }))
    }

    /// Delete the row. `Ok(())` whether or not it existed.
    ///
    /// # Errors
    ///
    /// [`PushError::Db`] on database failure.
    pub async fn delete(&self, project_id: ProjectId, kind: ProviderKind) -> Result<(), PushError> {
        sqlx::query("DELETE FROM push_credentials WHERE project_id = $1 AND kind = $2")
            .bind(project_id.into_uuid())
            .bind(kind.as_db_str())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Stamp `last_validated_at` + `last_validate_status` from
    /// a [`ValidateOutcome`].
    ///
    /// # Errors
    ///
    /// [`PushError::Db`] on database failure.
    pub async fn record_validate(
        &self,
        project_id: ProjectId,
        kind: ProviderKind,
        outcome: &ValidateOutcome,
    ) -> Result<(), PushError> {
        sqlx::query(
            r"
            UPDATE push_credentials
            SET last_validated_at = now(),
                last_validate_status = $1
            WHERE project_id = $2 AND kind = $3
            ",
        )
        .bind(outcome.as_db_str())
        .bind(project_id.into_uuid())
        .bind(kind.as_db_str())
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

fn translate_fk(err: sqlx::Error, project_id: ProjectId) -> PushError {
    if let sqlx::Error::Database(db_err) = &err
        && db_err.code().as_deref() == Some("23503")
    {
        return PushError::ProjectNotFound(project_id.into_uuid());
    }
    PushError::Db(err)
}
