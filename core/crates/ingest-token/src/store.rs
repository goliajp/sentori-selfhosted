//! `tokens` table CRUD.

use sqlx::{PgPool, Row};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::TokenError;
use crate::model::{Scope, Token};
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
    /// **The plaintext is returned ONCE** — DB stores only the hash.
    ///
    /// # Errors
    ///
    /// [`TokenError::Db`] on backend failure.
    pub async fn create(
        &self,
        project_id: Uuid,
        scope: Scope,
        name: &str,
    ) -> Result<(Uuid, String), TokenError> {
        let plaintext = mint_random_token();
        let token_hash = hash_token(&plaintext);
        let last4 = plaintext.chars().rev().take(4).collect::<String>();
        let last4: String = last4.chars().rev().collect();
        let id = Uuid::now_v7();

        sqlx::query(
            "INSERT INTO tokens (id, project_id, scope, name, token_hash, last4) \
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(id)
        .bind(project_id)
        .bind(scope.as_db_str())
        .bind(name)
        .bind(&token_hash)
        .bind(&last4)
        .execute(&self.pool)
        .await?;

        Ok((id, plaintext))
    }

    /// Look up a token by plaintext value. Returns the row regardless
    /// of scope — caller checks [`Token::is_active`] + scope.
    ///
    /// # Errors
    ///
    /// [`TokenError::Db`] on backend failure.
    pub async fn lookup_by_plaintext(&self, plaintext: &str) -> Result<Option<Token>, TokenError> {
        let token_hash = hash_token(plaintext);
        let row = sqlx::query(
            "SELECT id, project_id, scope, name, last4, created_at, revoked_at \
             FROM tokens WHERE token_hash = $1",
        )
        .bind(&token_hash)
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = row else { return Ok(None) };
        row_to_token(&row).map(Some)
    }

    /// List tokens for a project (UI dashboard).
    ///
    /// # Errors
    ///
    /// [`TokenError::Db`] on backend failure.
    pub async fn list_for_project(&self, project_id: Uuid) -> Result<Vec<Token>, TokenError> {
        let rows = sqlx::query(
            "SELECT id, project_id, scope, name, last4, created_at, revoked_at \
             FROM tokens WHERE project_id = $1 ORDER BY created_at DESC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(row_to_token).collect()
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

    /// Stamp `last_used_at`, best-effort — auth must not fail on it.
    pub async fn touch(&self, id: Uuid) {
        let _ = sqlx::query("UPDATE tokens SET last_used_at = now() WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await;
    }
}

fn row_to_token(row: &sqlx::postgres::PgRow) -> Result<Token, TokenError> {
    let scope_str: &str = row.get("scope");
    let scope = Scope::from_db_str(scope_str).ok_or_else(|| {
        TokenError::Db(sqlx::Error::Protocol(format!(
            "invalid token scope in DB: {scope_str}"
        )))
    })?;
    Ok(Token {
        id: row.get("id"),
        project_id: row.get("project_id"),
        scope,
        name: row.get("name"),
        last4: row.get("last4"),
        created_at: row.get::<OffsetDateTime, _>("created_at"),
        revoked_at: row.get::<Option<OffsetDateTime>, _>("revoked_at"),
    })
}

/// Generate a fresh `st_<26 base32>` plaintext token: 16 bytes of
/// crypto-random entropy as 26 unpadded base32 chars.
fn mint_random_token() -> String {
    use data_encoding::BASE32_NOPAD;
    use rand::Rng;
    let mut bytes = [0u8; 16];
    rand::rng().fill_bytes(&mut bytes);
    let encoded = BASE32_NOPAD.encode(&bytes).to_ascii_lowercase();
    format!("{}{}", crate::parse::TOKEN_PREFIX, encoded)
}
