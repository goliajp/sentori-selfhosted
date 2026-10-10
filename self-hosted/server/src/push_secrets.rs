//! Push secrets are sealed per project, provider and immutable credential identity.
use sentori_secrets_vault::Vault;
use serde_json::{Value, json};
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub fn seal(
    vault: &Vault,
    project: Uuid,
    id: Uuid,
    kind: &str,
    secret: &str,
) -> Result<Vec<u8>, &'static str> {
    let bytes = serde_json::to_vec(&json!({"project":project,"id":id,"kind":kind,"secret":secret}))
        .map_err(|_| "invalid_credential")?;
    vault.seal(&bytes).map_err(|_| "encryption_failed")
}
pub fn open(
    vault: Option<&Vault>,
    project: Uuid,
    id: Uuid,
    kind: &str,
    format: &str,
    blob: &[u8],
) -> Result<String, &'static str> {
    // Existing installations keep sending while the operator provisions a key.
    // Only explicitly tagged pre-v5 rows may take this upgrade path.
    if format == "legacy" {
        return String::from_utf8(blob.to_vec()).map_err(|_| "invalid_credential");
    }
    if format != "sealed" {
        return Err("invalid_credential_format");
    }
    let bytes = vault
        .ok_or("vault_unconfigured")?
        .open(blob)
        .map_err(|_| "decryption_failed")?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| "decryption_failed")?;
    if value["project"].as_str() != Some(project.to_string().as_str())
        || value["id"].as_str() != Some(id.to_string().as_str())
        || value["kind"].as_str() != Some(kind)
    {
        return Err("credential_scope_mismatch");
    }
    value["secret"]
        .as_str()
        .map(str::to_owned)
        .ok_or("decryption_failed")
}
pub async fn migrate(pool: &PgPool, vault: Option<&Vault>) -> Result<(), sqlx::Error> {
    let Some(vault) = vault else { return Ok(()) };
    let mut tx = pool.begin().await?;
    let rows=sqlx::query("SELECT id,project_id,kind,secret_blob FROM push_credentials WHERE secret_format='legacy' ORDER BY id LIMIT 64 FOR UPDATE SKIP LOCKED").fetch_all(&mut *tx).await?;
    for row in rows {
        let id: Uuid = row.get("id");
        let project: Uuid = row.get("project_id");
        let kind: String = row.get("kind");
        let blob: Vec<u8> = row.get("secret_blob");
        let secret = String::from_utf8(blob)
            .map_err(|_| sqlx::Error::Protocol("legacy push credential is not UTF-8".into()))?;
        let sealed = seal(vault, project, id, &kind, &secret)
            .map_err(|code| sqlx::Error::Protocol(code.into()))?;
        sqlx::query(
            "UPDATE push_credentials SET secret_blob=$2,secret_format='sealed' WHERE id=$1",
        )
        .bind(id)
        .bind(sealed)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await
}

pub fn spawn(state: std::sync::Arc<crate::state::AppState>) {
    tokio::spawn(async move {
        loop {
            if let Err(error) = migrate(&state.pool, state.vault.as_deref()).await {
                tracing::warn!(%error,"push credential encryption migration failed");
            }
            if let Err(error)=sqlx::query("DELETE FROM push_audience_previews WHERE id IN (SELECT id FROM push_audience_previews WHERE expires_at<now() ORDER BY expires_at LIMIT 1000)").execute(&state.pool).await {tracing::warn!(%error,"push preview expiry failed");}
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ciphertext_cannot_move_between_credentials_or_projects() -> Result<(), &'static str> {
        let key = sentori_secrets_vault::KeyId::new("test").map_err(|_| "key")?;
        let vault = Vault::new(sentori_secrets_vault::MasterKey::from_bytes([7; 32]), key);
        let project = Uuid::now_v7();
        let id = Uuid::now_v7();
        let blob = seal(&vault, project, id, "apns", "private fixture")?;
        assert_eq!(
            open(Some(&vault), project, id, "apns", "sealed", &blob)?,
            "private fixture"
        );
        assert!(open(Some(&vault), Uuid::now_v7(), id, "apns", "sealed", &blob).is_err());
        assert!(
            open(
                Some(&vault),
                project,
                Uuid::now_v7(),
                "apns",
                "sealed",
                &blob
            )
            .is_err()
        );
        assert!(open(Some(&vault), project, id, "fcm", "sealed", &blob).is_err());
        assert!(open(None, project, id, "apns", "sealed", &blob).is_err());
        Ok(())
    }
}
