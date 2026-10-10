//! Bounded, authenticated disaster-recovery snapshots shared by the server and operator CLI.
#![forbid(unsafe_code)]

pub mod archive;
pub mod capture;
pub mod healthcheck;
pub mod migration;
pub mod project_import;
pub mod reset;
pub mod restore;
mod schema;
pub mod verify;
pub use schema::TABLES;
pub use sentori_secrets_vault::MasterKey as RecoveryKey;

pub fn expected_migrations() -> Vec<archive::Migration> {
    sqlx::migrate!("../../core/migrations")
        .iter()
        .map(|migration| archive::Migration {
            version: migration.version,
            checksum: hex::encode(&migration.checksum),
        })
        .collect()
}

/// Parse an operator-held 256-bit key; passwords are deliberately not accepted.
pub fn parse_key(value: &str) -> anyhow::Result<RecoveryKey> {
    let bytes: [u8; 32] = hex::decode(value.trim())?
        .try_into()
        .map_err(|_| anyhow::anyhow!("recovery key must contain 64 hexadecimal characters"))?;
    Ok(RecoveryKey::from_bytes(bytes))
}

pub fn master_key_fingerprint(value: Option<&str>) -> anyhow::Result<Option<String>> {
    value
        .filter(|v| !v.is_empty())
        .map(|value| {
            let key = parse_key(value)?;
            Ok(fingerprint_key(&key))
        })
        .transpose()
}

pub fn fingerprint_key(key: &RecoveryKey) -> String {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    hash.update(b"sentori-backup-master-key-v1\0");
    hash.update(key.as_bytes());
    hex::encode(hash.finalize())
}

/// Coordinates consistent snapshots with physical blob garbage collection.
pub const BLOB_GC_LOCK: i64 = 0x5345_4e54_424c_4f42;
/// Shared by running servers; offline restore requires the exclusive counterpart.
pub const INSTANCE_SERVER_LOCK: i64 = 0x5345_4e54_5352_5652;
