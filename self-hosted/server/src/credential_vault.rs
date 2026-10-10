//! Optional operator-held key. No default key and no plaintext database fallback.
use sentori_secrets_vault::{KeyId, MasterKey, Vault};

pub fn from_env() -> Option<Vault> {
    let value = crate::env_config::env_or_file("SENTORI_MASTER_KEY")?;
    if value.is_empty() {
        return None;
    }
    let Some(bytes): Option<[u8; 32]> = hex::decode(value).ok().and_then(|v| v.try_into().ok())
    else {
        tracing::error!(
            "SENTORI_MASTER_KEY must contain 64 hexadecimal characters; encrypted integrations are disabled"
        );
        return None;
    };
    let Ok(id) = KeyId::new("sentori-v1") else {
        return None;
    };
    Some(Vault::new(MasterKey::from_bytes(bytes), id))
}
