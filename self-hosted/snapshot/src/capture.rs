//! Consistent PostgreSQL snapshot and referenced blob bytes. Call on a blocking
//! worker: encryption and bounded filesystem frames use synchronous I/O.
use std::{
    collections::BTreeSet,
    fs::File,
    io::{BufReader, BufWriter, Read, Write},
    path::Path,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};

use anyhow::{Context, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use futures_util::TryStreamExt;
use sentori_secrets_vault::{KeyId, MasterKey, Vault};
use sha2::{Digest, Sha256};
use sqlx::{AssertSqlSafe, Connection, PgConnection, PgPool, Row};

use crate::{
    BLOB_GC_LOCK, TABLES,
    archive::{
        ArchiveWriter, CHUNK_SIZE, Column, Header, Migration, Mode, Record, TransferBinding,
        TransferPhase,
    },
    verify::{Manifest, valid_hash, verify_controlled},
};

pub async fn migrations(conn: &mut PgConnection) -> anyhow::Result<Vec<Migration>> {
    let rows = sqlx::query("SELECT version, encode(checksum,'hex') AS checksum FROM _sqlx_migrations WHERE success ORDER BY version")
        .fetch_all(conn).await?;
    ensure!(!rows.is_empty(), "database has no applied migrations");
    Ok(rows
        .into_iter()
        .map(|row| Migration {
            version: row.get("version"),
            checksum: row.get("checksum"),
        })
        .collect())
}

pub async fn columns(conn: &mut PgConnection, table: &str) -> anyhow::Result<Vec<Column>> {
    ensure!(TABLES.contains(&table), "unknown snapshot table");
    let columns = sqlx::query_as("SELECT a.attname::text AS name,pg_catalog.format_type(a.atttypid,a.atttypmod) AS data_type,NOT a.attnotnull AS nullable,pg_catalog.pg_get_expr(d.adbin,d.adrelid) AS default_expression FROM pg_catalog.pg_attribute a JOIN pg_catalog.pg_class c ON c.oid=a.attrelid JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace LEFT JOIN pg_catalog.pg_attrdef d ON d.adrelid=c.oid AND d.adnum=a.attnum WHERE n.nspname='public' AND c.relname=$1 AND a.attnum>0 AND NOT a.attisdropped ORDER BY a.attnum")
        .bind(table).fetch_all(conn).await?;
    ensure!(!columns.is_empty(), "required table is missing: {table}");
    Ok(columns)
}

/// Capture to a new path; incomplete files are never published. The recovery
/// key is supplied by the operator and never written into the archive.
pub async fn capture(
    pool: &PgPool,
    blob_root: &Path,
    destination: &Path,
    key: MasterKey,
    application_version: &str,
    master_key: Option<MasterKey>,
) -> anyhow::Result<Manifest> {
    capture_controlled(
        pool,
        blob_root,
        destination,
        key,
        application_version,
        master_key,
        &CaptureControl::default(),
    )
    .await
}

/// Thread-safe bounded progress and cancellation at every row/blob frame.
#[derive(Default)]
pub struct CaptureControl {
    pub cancelled: AtomicBool,
    pub rows: AtomicU64,
    pub blob_bytes: AtomicU64,
    pub verifying: AtomicBool,
}
impl CaptureControl {
    pub fn check(&self) -> anyhow::Result<()> {
        ensure!(
            !self.cancelled.load(Ordering::Relaxed),
            "snapshot cancelled"
        );
        Ok(())
    }
}

pub async fn capture_controlled(
    pool: &PgPool,
    blob_root: &Path,
    destination: &Path,
    key: MasterKey,
    application_version: &str,
    master_key: Option<MasterKey>,
    control: &CaptureControl,
) -> anyhow::Result<Manifest> {
    capture_impl(
        pool,
        blob_root,
        destination,
        key,
        application_version,
        CaptureScope::Recovery(master_key),
        control,
    )
    .await
}

pub async fn capture_projects(
    pool: &PgPool,
    blob_root: &Path,
    destination: &Path,
    key: MasterKey,
    application_version: &str,
    projects: &[uuid::Uuid],
    control: &CaptureControl,
) -> anyhow::Result<Manifest> {
    crate::migration::Scope::new(projects)?;
    capture_impl(
        pool,
        blob_root,
        destination,
        key,
        application_version,
        CaptureScope::Projects(projects, None),
        control,
    )
    .await
}

/// A managed transfer package binds its destination and phase to the durable
/// source receipt. A final package requires every selected project to be fenced.
pub async fn capture_transfer(
    pool: &PgPool,
    blob_root: &Path,
    destination: &Path,
    key: MasterKey,
    application_version: &str,
    binding: &TransferBinding,
    control: &CaptureControl,
) -> anyhow::Result<Manifest> {
    let projects: Vec<uuid::Uuid> = sqlx::query_scalar(
        "SELECT project_ids FROM project_transfers WHERE id=$1 AND direction='source'",
    )
    .bind(binding.id)
    .fetch_one(pool)
    .await?;
    crate::migration::Scope::new(&projects)?;
    capture_impl(
        pool,
        blob_root,
        destination,
        key,
        application_version,
        CaptureScope::Projects(&projects, Some(binding)),
        control,
    )
    .await
}

enum CaptureScope<'a> {
    Recovery(Option<MasterKey>),
    OfflineReset(Option<MasterKey>),
    Projects(&'a [uuid::Uuid], Option<&'a TransferBinding>),
}

/// Only the reset coordinator may use this path: its live transaction holds
/// BLOB_GC_LOCK exclusively and SHARE locks on every table until after capture.
/// Taking another GC lock here would deadlock against that same transaction.
pub(crate) async fn capture_for_reset(
    pool: &PgPool,
    blob_root: &Path,
    destination: &Path,
    key: MasterKey,
    application_version: &str,
    master_key: Option<MasterKey>,
) -> anyhow::Result<Manifest> {
    capture_impl(
        pool,
        blob_root,
        destination,
        key,
        application_version,
        CaptureScope::OfflineReset(master_key),
        &CaptureControl::default(),
    )
    .await
}

async fn capture_impl(
    pool: &PgPool,
    blob_root: &Path,
    destination: &Path,
    key: MasterKey,
    application_version: &str,
    scope: CaptureScope<'_>,
    control: &CaptureControl,
) -> anyhow::Result<Manifest> {
    let (master_key, projects, binding, gc_already_held) = match scope {
        CaptureScope::Recovery(key) => (key, None, None, false),
        CaptureScope::OfflineReset(key) => (key, None, None, true),
        CaptureScope::Projects(projects, binding) => (None, Some(projects), binding, false),
    };
    control.check()?;
    ensure!(blob_root.is_dir(), "durable blob directory required");
    ensure!(!destination.exists(), "snapshot destination already exists");
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    // A detached physical connection closes rather than returning a session
    // advisory lock to a pool on error/cancellation. Obtain this lock BEFORE
    // opening the repeatable-read snapshot, including when GC is already busy.
    let mut conn = pool.acquire().await?.detach();
    sqlx::query("SET lock_timeout = '10s'")
        .execute(&mut conn)
        .await?;
    if !gc_already_held {
        sqlx::query("SELECT pg_advisory_lock_shared($1)")
            .bind(BLOB_GC_LOCK)
            .execute(&mut conn)
            .await?;
    }
    let mut tx = conn.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let schema = migrations(&mut tx).await?;
    let master_key_fingerprint = master_key.as_ref().map(crate::fingerprint_key);
    let master_vault = master_key
        .map(|key| KeyId::new("sentori-v1").map(|id| Vault::new(key, id)))
        .transpose()?;
    let instance = sqlx::query("SELECT id,generation FROM instance_settings WHERE singleton")
        .fetch_one(&mut *tx)
        .await?;
    if let Some(binding) = binding {
        validate_transfer(
            &mut tx,
            instance.get("id"),
            instance.get("generation"),
            projects.unwrap_or_default(),
            binding,
        )
        .await?;
    }
    let header = Header {
        format: 1,
        archive_id: uuid::Uuid::new_v4(),
        instance_id: instance.get("id"),
        generation: instance.get("generation"),
        created_at: time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)?,
        application_version: application_version.to_owned(),
        migrations: schema,
        master_key_fingerprint,
        mode: if projects.is_some() {
            Mode::ProjectMigration
        } else {
            Mode::DisasterRecovery
        },
        project_ids: projects.unwrap_or_default().to_vec(),
        transfer: binding.cloned(),
    };
    let mut writer =
        ArchiveWriter::new(BufWriter::new(temporary.as_file_mut()), key.clone(), header)?;
    let mut referenced = BTreeSet::new();
    let tables = if projects.is_some() {
        crate::migration::TABLES
    } else {
        TABLES
    };
    for table in tables {
        control.check()?;
        let mut columns = columns(&mut tx, table).await?;
        if projects.is_some() {
            let allowed = crate::migration::columns(table);
            columns.retain(|c| allowed.contains(&c.name.as_str()));
            ensure!(
                columns.len() == allowed.len(),
                "migration schema missing required columns"
            );
        }
        writer.write(Record::Table {
            name: (*table).to_owned(),
            columns,
        })?;
        // Audited: table is exclusively from the compile-time TABLES allowlist.
        let query = if projects.is_some() {
            crate::migration::query(table)?
        } else {
            format!("SELECT row_to_json(t) AS row FROM {table} t")
        };
        let mut query = sqlx::query(AssertSqlSafe(query));
        if let Some(projects) = projects {
            query = query.bind(projects);
        }
        let mut rows = query.fetch(&mut *tx);
        let mut count = 0;
        let mut digest = Sha256::new();
        while let Some(row) = rows.try_next().await? {
            control.check()?;
            let value: serde_json::Value = row.get("row");
            if *table == "cloud_instance_binding" {
                ensure!(
                    master_vault.is_some(),
                    "Cloud recovery requires the instance master key"
                );
            }
            let secret_column = match *table {
                "projects" => Some("webhook_secret_blob"),
                "task_integrations" | "outbound_integrations" => Some("secret_blob"),
                "instance_operations" | "project_transfer_jobs" => Some("key_blob"),
                "project_transfer_routes" => Some("sealed_token"),
                "auth_sessions" => Some("sealed_cloud_grant"),
                "push_credentials" if value["secret_format"] == "sealed" => Some("secret_blob"),
                _ => None,
            };
            if let Some(secret) = secret_column.and_then(|column| value[column].as_str()) {
                let vault = master_vault
                    .as_ref()
                    .context("SENTORI_MASTER_KEY required to validate encrypted credentials")?;
                let bytes = hex::decode(
                    secret
                        .strip_prefix("\\x")
                        .context("invalid encrypted credential encoding")?,
                )?;
                vault
                    .open(&bytes)
                    .context("instance master key cannot decrypt stored credentials")?;
            }
            let column = match *table {
                "event_attachments" => Some("blob_hash"),
                "release_artifacts" => Some("content_hash"),
                _ => None,
            };
            if let Some(column) = column {
                let hash = value[column].as_str().context("missing blob reference")?;
                ensure!(valid_hash(hash), "invalid blob hash in database");
                referenced.insert(hash.to_owned());
            }
            let bytes = serde_json::to_vec(&value)?;
            digest.update(&bytes);
            digest.update(b"\n");
            writer.write(Record::Row {
                table: (*table).to_owned(),
                value,
            })?;
            count += 1;
            control.rows.fetch_add(1, Ordering::Relaxed);
        }
        writer.write(Record::TableEnd {
            name: (*table).to_owned(),
            rows: count,
            sha256: hex::encode(digest.finalize()),
        })?;
    }
    for hash in referenced {
        control.check()?;
        let path = blob_root
            .join(&hash[..2])
            .join(format!("{}.bin", &hash[2..]));
        let mut file = File::open(path).context("referenced blob missing")?;
        ensure!(file.metadata()?.is_file(), "blob is not a regular file");
        let size = file.metadata()?.len();
        writer.write(Record::Blob {
            hash: hash.clone(),
            bytes: size,
        })?;
        let mut digest = Sha256::new();
        let mut position = 0;
        let mut chunk = vec![0; CHUNK_SIZE];
        loop {
            control.check()?;
            let length = file.read(&mut chunk)?;
            if length == 0 {
                break;
            }
            ensure!(
                position + length as u64 <= size,
                "blob changed during backup"
            );
            digest.update(&chunk[..length]);
            writer.write(Record::Chunk {
                hash: hash.clone(),
                offset: position,
                base64: STANDARD.encode(&chunk[..length]),
            })?;
            position += length as u64;
            control
                .blob_bytes
                .fetch_add(length as u64, Ordering::Relaxed);
        }
        ensure!(
            position == size && hex::encode(digest.finalize()) == hash,
            "referenced blob content is corrupt"
        );
        writer.write(Record::BlobEnd { hash })?;
    }
    writer.finish()?.flush()?;
    temporary.as_file().sync_all()?;
    tx.commit().await?;
    conn.close().await?;
    // Independent read verifies everything the writer actually persisted,
    // including the complete manifest and all referenced content.
    control.verifying.store(true, Ordering::Relaxed);
    let manifest = verify_controlled(BufReader::new(File::open(temporary.path())?), key, control)?;
    control.check()?;
    temporary
        .persist_noclobber(destination)
        .map_err(|e| e.error)?;
    File::open(parent)?.sync_all()?;
    Ok(manifest)
}

async fn validate_transfer(
    conn: &mut PgConnection,
    instance: uuid::Uuid,
    generation: i64,
    projects: &[uuid::Uuid],
    binding: &TransferBinding,
) -> anyhow::Result<()> {
    let receipt: (uuid::Uuid,i64,uuid::Uuid,i64,Vec<uuid::Uuid>,String) = sqlx::query_as("SELECT source_instance_id,source_generation,target_instance_id,target_generation,project_ids,state FROM project_transfers WHERE id=$1 AND direction='source'")
        .bind(binding.id).fetch_one(&mut *conn).await?;
    ensure!(
        receipt.0 == instance
            && receipt.1 == generation
            && receipt.2 == binding.target_instance_id
            && receipt.3 == binding.target_generation
            && receipt.4 == projects,
        "source transfer binding changed"
    );
    let final_copy = binding.phase == TransferPhase::Final;
    ensure!(
        if final_copy {
            receipt.5 == "sealed"
        } else {
            matches!(receipt.5.as_str(), "copying" | "copied")
        },
        "source transfer is not in the requested phase"
    );
    let matching:i64=sqlx::query_scalar("SELECT count(*) FROM projects WHERE id=ANY($1) AND migration_fence IS NOT DISTINCT FROM $2")
        .bind(projects).bind(final_copy.then_some(binding.id)).fetch_one(conn).await?;
    ensure!(
        usize::try_from(matching)? == projects.len(),
        "source transfer fence does not cover the exact scope"
    );
    Ok(())
}
