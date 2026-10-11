//! Verified project imports into newly allocated destination projects. This
//! module never imports credentials, grants or automation and never commits the
//! caller's transaction. Its caller owns authorization, quotas and cutover state.
use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom, Write},
    path::Path,
};

use anyhow::{Context, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use futures_util::TryStreamExt;
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{AssertSqlSafe, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    RecoveryKey,
    archive::{ArchiveReader, MAX_ARCHIVE, Mode, Record},
    capture::CaptureControl,
    migration,
    verify::{Manifest, verify_controlled},
};

/// Namespace every server-generated identity. Event/session/attachment IDs stay
/// unchanged so delayed SDK retries still deduplicate after routing to Cloud.
pub fn mapped_id(transfer: Uuid, table: &str, source: Uuid) -> Uuid {
    let mut digest = Sha256::new();
    digest.update(b"sentori-project-transfer-v1\0");
    digest.update(transfer.as_bytes());
    digest.update(table.as_bytes());
    digest.update([0]);
    digest.update(source.as_bytes());
    let hash = digest.finalize();
    let mut bytes = [0; 16];
    bytes.copy_from_slice(&hash[..16]);
    // UUIDv8: application-defined deterministic identities, RFC variant.
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

fn transform(transfer: Uuid, table: &str, mut value: Value) -> anyhow::Result<Value> {
    for (column, domain) in [
        ("project_id", "projects"),
        ("app_id", "apps"),
        ("issue_id", "issues"),
        ("release_id", "releases"),
        ("task_id", "tasks"),
        ("actor_user_id", "users"),
        ("assignee_user_id", "users"),
        ("created_by", "users"),
        ("id", table),
    ] {
        if value[column].is_null() || (column == "id" && matches!(table, "events" | "sessions")) {
            continue;
        }
        let original: Uuid = value[column]
            .as_str()
            .context("invalid transfer identity")?
            .parse()?;
        value[column] = json!(mapped_id(transfer, domain, original));
    }
    Ok(value)
}

/// Multiset checksum for database readback, independent of PostgreSQL row order.
/// Primary/unique constraints reject identical duplicate rows. The encrypted
/// archive's ordered SHA-256 transcript is independently checked before writes.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct RowChecksum {
    pub rows: u64,
    pub xor_sha256: [u8; 32],
}
impl RowChecksum {
    fn add(&mut self, row: &Value) -> anyhow::Result<()> {
        self.rows = self.rows.checked_add(1).context("too many imported rows")?;
        for (out, byte) in self
            .xor_sha256
            .iter_mut()
            .zip(Sha256::digest(serde_json::to_vec(row)?))
        {
            *out ^= byte;
        }
        Ok(())
    }
}

/// Pin the encrypted input to a private file before verification, so a changed
/// upload path cannot replace already verified content during the import pass.
pub struct PreparedImport {
    file: tempfile::NamedTempFile,
    key: RecoveryKey,
    pub manifest: Manifest,
}
impl PreparedImport {
    pub fn open(source: &Path, key: RecoveryKey, control: &CaptureControl) -> anyhow::Result<Self> {
        let source = File::open(source)?;
        ensure!(source.metadata()?.len() <= MAX_ARCHIVE, "archive too large");
        let mut file = tempfile::NamedTempFile::new()?;
        let mut source = source.take(MAX_ARCHIVE + 1);
        let mut bytes = 0u64;
        let mut buffer = vec![0; crate::archive::CHUNK_SIZE];
        loop {
            control.check()?;
            let count = source.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            bytes += u64::try_from(count)?;
            ensure!(bytes <= MAX_ARCHIVE, "archive grew beyond the size limit");
            file.write_all(&buffer[..count])?;
        }
        file.as_file_mut().seek(SeekFrom::Start(0))?;
        let manifest = verify_controlled(BufReader::new(file.as_file_mut()), key.clone(), control)?;
        ensure!(
            manifest.header.mode == Mode::ProjectMigration,
            "only a project migration package can be imported"
        );
        Ok(Self {
            file,
            key,
            manifest,
        })
    }

    pub fn project_mapping(&self, transfer: Uuid) -> BTreeMap<Uuid, Uuid> {
        self.manifest
            .header
            .project_ids
            .iter()
            .map(|id| (*id, mapped_id(transfer, "projects", *id)))
            .collect()
    }

    /// Requires a live caller-owned transaction, with its transfer/quota locks
    /// already held. New blobs may remain unreferenced on rollback; ordinary GC
    /// reclaims them. Existing blobs are never overwritten or removed here.
    pub async fn apply(
        &mut self,
        tx: &mut Transaction<'_, Postgres>,
        blob_root: &Path,
        transfer: Uuid,
        control: &CaptureControl,
    ) -> anyhow::Result<BTreeMap<String, RowChecksum>> {
        ensure!(
            blob_root.is_dir(),
            "durable destination blob store required"
        );
        if let Some(binding) = &self.manifest.header.transfer {
            let target: (Uuid, i64) =
                sqlx::query_as("SELECT id,generation FROM instance_settings WHERE singleton")
                    .fetch_one(&mut **tx)
                    .await?;
            ensure!(
                binding.id == transfer
                    && target == (binding.target_instance_id, binding.target_generation),
                "transfer target identity, generation or operation differs"
            );
        }
        sqlx::query("SELECT pg_advisory_xact_lock_shared($1)")
            .bind(crate::BLOB_GC_LOCK)
            .execute(&mut **tx)
            .await?;
        ensure!(
            crate::capture::migrations(tx).await? == self.manifest.header.migrations,
            "destination schema differs from the verified package"
        );
        let projects: Vec<Uuid> = self.project_mapping(transfer).into_values().collect();
        let occupied: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM projects WHERE id=ANY($1))")
                .bind(&projects)
                .fetch_one(&mut **tx)
                .await?;
        ensure!(!occupied, "destination projects already exist");
        self.file.as_file_mut().seek(SeekFrom::Start(0))?;
        let mut reader =
            ArchiveReader::new(BufReader::new(self.file.as_file_mut()), self.key.clone())?;
        let mut expected: BTreeMap<String, RowChecksum> = migration::TABLES
            .iter()
            .map(|table| ((*table).to_owned(), RowChecksum::default()))
            .collect();
        let mut blob: Option<(String, tempfile::NamedTempFile)> = None;
        while let Some(record) = reader.read_record()? {
            control.check()?;
            match record {
                Record::Row { table, value } => {
                    ensure!(
                        migration::TABLES.contains(&table.as_str()),
                        "unknown import table"
                    );
                    let value = transform(transfer, &table, value)?;
                    let written = insert_row(tx, &table, value.clone()).await?;
                    let columns = &self.manifest.tables[&table].columns;
                    ensure!(
                        canonical(value, columns)? == canonical(written.clone(), columns)?,
                        "database changed imported fields for {table}"
                    );
                    expected
                        .get_mut(&table)
                        .context("missing import table")?
                        .add(&written)?;
                    control
                        .rows
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
                Record::Blob { hash, .. } => {
                    ensure!(crate::verify::valid_hash(&hash), "invalid blob identifier");
                    let directory = blob_root.join(&hash[..2]);
                    std::fs::create_dir_all(&directory)?;
                    ensure!(
                        !std::fs::symlink_metadata(&directory)?
                            .file_type()
                            .is_symlink(),
                        "blob directory is a symlink"
                    );
                    blob = Some((hash, tempfile::NamedTempFile::new_in(directory)?));
                }
                Record::Chunk { base64, .. } => {
                    let bytes = STANDARD.decode(base64)?;
                    blob.as_mut()
                        .context("missing import blob")?
                        .1
                        .write_all(&bytes)?;
                    control.blob_bytes.fetch_add(
                        u64::try_from(bytes.len())?,
                        std::sync::atomic::Ordering::Relaxed,
                    );
                }
                Record::BlobEnd { hash } => {
                    let (active, file) = blob.take().context("missing import blob")?;
                    ensure!(active == hash, "import blob identity changed");
                    file.as_file().sync_all()?;
                    let destination = blob_root
                        .join(&hash[..2])
                        .join(format!("{}.bin", &hash[2..]));
                    match file.persist_noclobber(&destination) {
                        Ok(_) => {}
                        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {}
                        Err(error) => return Err(error.error.into()),
                    }
                    verify_blob(&destination, &hash, self.manifest.blobs[&hash])?;
                    File::open(blob_root.join(&hash[..2]))?.sync_all()?;
                }
                _ => {}
            }
        }
        File::open(blob_root)?.sync_all()?;
        for table in migration::TABLES {
            // Audited: query and columns come only from migration's fixed allowlist.
            let query = migration::query(table)?;
            let mut rows = sqlx::query_scalar::<_, Value>(AssertSqlSafe(query))
                .bind(&projects)
                .fetch(&mut **tx);
            let mut actual = RowChecksum::default();
            while let Some(row) = rows.try_next().await? {
                control.check()?;
                actual.add(&row)?;
            }
            ensure!(
                actual == expected[*table],
                "destination readback differs for {table}"
            );
            ensure!(
                actual.rows == self.manifest.tables[*table].rows,
                "import row count changed for {table}"
            );
        }
        Ok(expected)
    }
}

fn canonical(mut row: Value, columns: &[crate::archive::Column]) -> anyhow::Result<Value> {
    for column in columns {
        if column.data_type == "timestamp with time zone" && !row[&column.name].is_null() {
            let parsed = time::OffsetDateTime::parse(
                row[&column.name]
                    .as_str()
                    .context("invalid imported timestamp")?,
                &time::format_description::well_known::Rfc3339,
            )?;
            row[&column.name] = json!(parsed.unix_timestamp_nanos().to_string());
        }
    }
    Ok(row)
}

fn verify_blob(path: &Path, expected: &str, bytes: u64) -> anyhow::Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    ensure!(
        metadata.is_file() && metadata.len() == bytes,
        "destination blob shape differs"
    );
    let mut source = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = vec![0; crate::archive::CHUNK_SIZE];
    loop {
        let count = source.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    ensure!(
        hex::encode(digest.finalize()) == expected,
        "destination blob checksum differs"
    );
    Ok(())
}

async fn insert_row(
    tx: &mut Transaction<'_, Postgres>,
    table: &str,
    mut value: Value,
) -> anyhow::Result<Value> {
    let allowed = migration::columns(table);
    ensure!(!allowed.is_empty(), "unknown import table");
    let mut columns = allowed.to_vec();
    let mut conflict = "";
    if table == "users" {
        let id = value["id"].as_str().context("missing principal identity")?;
        value["email"] = json!(format!("archived-{id}@migration.invalid"));
        value["password_hash"] = json!("!migration");
        value["role"] = json!("admin");
        columns.extend(["email", "password_hash", "role"]);
        // A final package updates historical display names, never a real account.
        conflict = " ON CONFLICT(id) DO UPDATE SET display_name=excluded.display_name WHERE users.password_hash='!migration' AND users.email=excluded.email AND users.role='admin'";
    } else if table == "task_events" {
        // Historical activities do not recreate inbox notifications.
        value["category"] = json!("task");
        columns.push("category");
    }
    // Audited: table and all identifiers are fixed positive-list literals. Every
    // archive value is a bound parameter, including text that resembles SQL.
    let sql = format!(
        "INSERT INTO {table} ({columns}) SELECT {columns} FROM json_populate_record(NULL::{table}, $1::json){conflict} RETURNING to_jsonb({table})",
        columns = columns.join(",")
    );
    let mut written: Value = sqlx::query_scalar(AssertSqlSafe(sql))
        .bind(value.to_string())
        .fetch_one(&mut **tx)
        .await?;
    written
        .as_object_mut()
        .context("import returning row was not an object")?
        .retain(|key, _| allowed.contains(&key.as_str()));
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_only_server_identities_and_preserves_sdk_deduplication() -> anyhow::Result<()> {
        let transfer = Uuid::new_v4();
        let id = Uuid::new_v4();
        let event = transform(
            transfer,
            "events",
            json!({"id":id,"project_id":id,"issue_id":id,"payload":{"id":id}}),
        )?;
        assert_eq!(event["id"], id.to_string());
        assert_eq!(event["payload"]["id"], id.to_string());
        assert_ne!(event["project_id"], event["issue_id"]);
        assert_eq!(
            event["project_id"],
            mapped_id(transfer, "projects", id).to_string()
        );
        assert_ne!(
            mapped_id(transfer, "projects", id),
            mapped_id(Uuid::new_v4(), "projects", id)
        );
        let attachment = transform(
            transfer,
            "event_attachments",
            json!({"ref":id,"event_id":id,"project_id":id}),
        )?;
        assert_eq!(attachment["ref"], id.to_string());
        assert_eq!(attachment["event_id"], id.to_string());
        assert!(transform(transfer, "tasks", json!({"created_by":"invalid"})).is_err());
        Ok(())
    }

    #[test]
    fn readback_checksum_detects_changes_and_ignores_row_order() -> anyhow::Result<()> {
        let a = json!({"id":"a","value":1});
        let b = json!({"id":"b","value":2});
        let mut expected = RowChecksum::default();
        expected.add(&a)?;
        expected.add(&b)?;
        let mut actual = RowChecksum::default();
        actual.add(&b)?;
        actual.add(&a)?;
        assert_eq!(expected, actual);
        actual.add(&a)?;
        assert_ne!(expected, actual);
        Ok(())
    }
}
