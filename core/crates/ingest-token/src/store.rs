//! `tokens` table CRUD.

use sentori_workspace_identity::{ProjectId, WorkspaceId};
use sqlx::{PgPool, Row};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::TokenError;
use crate::model::{Token, TokenKind};
use crate::parse::hash_token;

#[derive(Clone, Debug)]
pub struct TokenStore {
    pool: PgPool,
}

impl TokenStore {
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Mint a new token. Returns `(token_id, plaintext_token)`.
    /// **The plaintext is returned ONCE** — caller is responsible
    /// for showing it to the user (typically in a `<code>` block
    /// they can copy). DB stores only the SHA-256 hash.
    ///
    /// # Errors
    ///
    /// [`TokenError::Db`] on backend failure.
    pub async fn create(
        &self,
        workspace_id: WorkspaceId,
        project_id: ProjectId,
        kind: TokenKind,
        label: Option<&str>,
    ) -> Result<(Uuid, String), TokenError> {
        let plaintext = mint_random_token();
        let token_hash = hash_token(&plaintext);
        let last4 = plaintext.chars().rev().take(4).collect::<String>();
        let last4: String = last4.chars().rev().collect();
        let id = Uuid::now_v7();

        sqlx::query(
            "INSERT INTO tokens (id, workspace_id, project_id, kind, token_hash, label, last4) \
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(id)
        .bind(workspace_id.into_uuid())
        .bind(project_id.into_uuid())
        .bind(kind.as_db_str())
        .bind(&token_hash)
        .bind(label)
        .bind(&last4)
        .execute(&self.pool)
        .await?;

        Ok((id, plaintext))
    }

    /// Look up a token by plaintext value. Returns the row
    /// regardless of kind — caller checks
    /// [`Token::is_active`] + kind.
    ///
    /// # Errors
    ///
    /// [`TokenError::Db`] on backend failure.
    pub async fn lookup_by_plaintext(&self, plaintext: &str) -> Result<Option<Token>, TokenError> {
        let token_hash = hash_token(plaintext);
        let row = sqlx::query(
            "SELECT id, workspace_id, project_id, kind, label, last4, created_at, revoked_at \
             FROM tokens WHERE token_hash = $1",
        )
        .bind(&token_hash)
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = row else { return Ok(None) };
        let kind_str: &str = row.get("kind");
        let kind = TokenKind::from_db_str(kind_str).ok_or_else(|| {
            TokenError::Db(sqlx::Error::Protocol(format!(
                "invalid token kind in DB: {kind_str}"
            )))
        })?;
        Ok(Some(Token {
            id: row.get("id"),
            workspace_id: WorkspaceId::from_uuid(row.get::<Uuid, _>("workspace_id")),
            project_id: ProjectId::from_uuid(row.get::<Uuid, _>("project_id")),
            kind,
            label: row.get("label"),
            last4: row.get("last4"),
            created_at: row.get::<OffsetDateTime, _>("created_at"),
            revoked_at: row.get::<Option<OffsetDateTime>, _>("revoked_at"),
        }))
    }

    /// List tokens for a project (UI dashboard).
    ///
    /// # Errors
    ///
    /// [`TokenError::Db`] on backend failure.
    pub async fn list_for_project(&self, project_id: ProjectId) -> Result<Vec<Token>, TokenError> {
        let rows = sqlx::query(
            "SELECT id, workspace_id, project_id, kind, label, last4, created_at, revoked_at \
             FROM tokens WHERE project_id = $1 ORDER BY created_at DESC",
        )
        .bind(project_id.into_uuid())
        .fetch_all(&self.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let kind_str: &str = r.get("kind");
            let kind = TokenKind::from_db_str(kind_str).ok_or_else(|| {
                TokenError::Db(sqlx::Error::Protocol(format!(
                    "invalid token kind: {kind_str}"
                )))
            })?;
            out.push(Token {
                id: r.get("id"),
                workspace_id: WorkspaceId::from_uuid(r.get::<Uuid, _>("workspace_id")),
                project_id: ProjectId::from_uuid(r.get::<Uuid, _>("project_id")),
                kind,
                label: r.get("label"),
                last4: r.get("last4"),
                created_at: r.get::<OffsetDateTime, _>("created_at"),
                revoked_at: r.get::<Option<OffsetDateTime>, _>("revoked_at"),
            });
        }
        Ok(out)
    }

    /// Soft-delete a token. Idempotent.
    ///
    /// # Errors
    ///
    /// [`TokenError::Db`] on backend failure.
    pub async fn revoke(&self, id: Uuid) -> Result<(), TokenError> {
        sqlx::query("UPDATE tokens SET revoked_at = now() WHERE id = $1 AND revoked_at IS NULL")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

/// Generate a fresh `st_pk_<26 base32>` plaintext token. Uses
/// 16 bytes of crypto-random entropy encoded as 26 base32 chars
/// (RFC 4648 unpadded).
fn mint_random_token() -> String {
    use data_encoding::BASE32_NOPAD;
    use rand::RngCore;
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    let encoded = BASE32_NOPAD.encode(&bytes).to_ascii_lowercase();
    // 16 bytes → 26 base32 chars
    format!("{}{}", crate::parse::TOKEN_PREFIX, encoded)
}
