//! Offline restore to an empty, migrated database and a new blob directory.
//! The target server must remain stopped until this operation succeeds.
use std::{
    fs::{File, OpenOptions},
    io::{BufReader, Read, Seek, SeekFrom, Write},
    path::Path,
};

use anyhow::{Context, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use sentori_secrets_vault::MasterKey;
use sha2::{Digest, Sha256};
use sqlx::{AssertSqlSafe, PgPool};

#[derive(serde::Serialize)]
pub struct RestoreReport {
    pub instance_id: uuid::Uuid,
    pub generation: i64,
    pub archive_id: uuid::Uuid,
    pub source_instance_id: uuid::Uuid,
}

use crate::{
    BLOB_GC_LOCK, TABLES,
    archive::{ArchiveReader, MAX_ARCHIVE, Record},
    capture::{columns, migrations},
    verify::verify,
};

/// Preflight completes before any target data changes. All database rows commit
/// together. Blob writes belong to a newly created directory, never an existing
/// store. On a returned error that directory is removed; after a process crash
/// an uncommitted directory is left for explicit operator inspection.
pub async fn restore(
    pool: &PgPool,
    source: &Path,
    destination: &Path,
    key: MasterKey,
    master_key_fingerprint: Option<&str>,
) -> anyhow::Result<RestoreReport> {
    ensure!(
        !destination.exists(),
        "restore requires a new blob directory"
    );
    let source = File::open(source).context("open recovery archive")?;
    ensure!(
        source.metadata()?.is_file() && source.metadata()?.len() <= MAX_ARCHIVE,
        "invalid archive file or size"
    );
    // Copy ciphertext to a private file so a replacement or in-place edit of the
    // uploaded source cannot turn a successful preflight into different writes.
    let mut stable = tempfile::tempfile()?;
    let copied = std::io::copy(&mut source.take(MAX_ARCHIVE + 1), &mut stable)?;
    ensure!(copied <= MAX_ARCHIVE, "archive exceeds size limit");
    stable.seek(SeekFrom::Start(0))?;
    let manifest = verify(BufReader::new(&mut stable), key.clone())?;
    ensure!(
        manifest.header.mode == crate::archive::Mode::DisasterRecovery,
        "project migration packages require a project importer, not instance restore"
    );
    ensure!(
        manifest.header.master_key_fingerprint.as_deref() == master_key_fingerprint,
        "the target instance master key does not match this recovery archive"
    );
    let mut tx = pool.begin().await?;
    let offline: bool = sqlx::query_scalar("SELECT pg_try_advisory_xact_lock($1)")
        .bind(crate::INSTANCE_SERVER_LOCK)
        .fetch_one(&mut *tx)
        .await?;
    ensure!(
        offline,
        "target server is running; stop it before restoring"
    );
    sqlx::query("SET LOCAL lock_timeout='10s'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(BLOB_GC_LOCK)
        .execute(&mut *tx)
        .await?;
    // Audited: table names are solely the fixed TABLES allowlist, never archive input.
    sqlx::query(AssertSqlSafe(format!(
        "LOCK TABLE {} IN ACCESS EXCLUSIVE MODE",
        TABLES.join(",")
    )))
    .execute(&mut *tx)
    .await?;
    sqlx::query("SET LOCAL sentori.offline_recovery='on'")
        .execute(&mut *tx)
        .await?;
    ensure!(
        migrations(&mut tx).await? == manifest.header.migrations,
        "schema versions or migration checksums differ"
    );
    for table in TABLES {
        ensure!(
            columns(&mut tx, table).await? == manifest.tables[*table].columns,
            "schema columns differ for {table}"
        );
        if *table == "instance_settings" {
            let initialized: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM instance_settings WHERE initialized_at IS NOT NULL)",
            )
            .fetch_one(&mut *tx)
            .await?;
            ensure!(!initialized, "restore target is already initialized");
        } else {
            // Audited: table is solely from TABLES, never archive input.
            let occupied: bool = sqlx::query_scalar(AssertSqlSafe(format!(
                "SELECT EXISTS(SELECT 1 FROM {table})"
            )))
            .fetch_one(&mut *tx)
            .await?;
            ensure!(
                !occupied,
                "restore requires empty target tables ({table} is populated)"
            );
        }
    }
    let target_id: uuid::Uuid =
        sqlx::query_scalar("SELECT id FROM instance_settings WHERE singleton")
            .fetch_one(&mut *tx)
            .await?;
    ensure!(
        target_id != manifest.header.instance_id,
        "restore target must be a distinct new instance"
    );
    let generation = manifest
        .header
        .generation
        .checked_add(1)
        .context("instance generation exhausted")?;
    // The directory is acquired with create_dir, not an existence check followed
    // by rename: another restore or an existing store cannot be overwritten.
    std::fs::create_dir(destination).context("claim new restore blob directory")?;
    let result = async {
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(destination,std::fs::Permissions::from_mode(0o700))?;
        }
        sqlx::query("DELETE FROM instance_settings").execute(&mut *tx).await?;
        stable.seek(SeekFrom::Start(0))?;
        let mut reader = ArchiveReader::new(BufReader::new(&mut stable), key)?;
        let mut blob: Option<File> = None;
        while let Some(record) = reader.read_record()? {
            match record {
                Record::Row { table, value } => {
                    ensure!(TABLES.contains(&table.as_str()), "unknown restore table");
                    // Audited: table was checked against TABLES immediately above.
                    sqlx::query(AssertSqlSafe(format!("INSERT INTO {table} SELECT * FROM json_populate_record(NULL::{table}, $1::json)")))
                        .bind(value.to_string()).execute(&mut *tx).await?;
                }
                Record::Blob { hash, .. } => {
                    ensure!(crate::verify::valid_hash(&hash), "invalid blob identifier");
                    let directory = destination.join(&hash[..2]);
                    std::fs::create_dir_all(&directory)?;
                    blob = Some(OpenOptions::new().write(true).create_new(true)
                        .open(directory.join(format!("{}.bin", &hash[2..])))?);
                }
                Record::Chunk { base64, .. } => {
                    blob.as_mut().context("missing blob file")?.write_all(&STANDARD.decode(base64)?)?;
                }
                Record::BlobEnd { .. } => {
                    blob.take().context("missing blob file")?.sync_all()?;
                }
                _ => {}
            }
        }
        for directory in std::fs::read_dir(destination)? {
            File::open(directory?.path())?.sync_all()?;
        }
        for (hash, expected_size) in &manifest.blobs {
            let mut file = File::open(destination.join(&hash[..2]).join(format!("{}.bin", &hash[2..])))?;
            ensure!(file.metadata()?.len() == *expected_size, "restored blob size mismatch");
            let mut digest = Sha256::new();
            let mut buffer = vec![0; crate::archive::CHUNK_SIZE];
            loop {
                let n = file.read(&mut buffer)?;
                if n == 0 { break; }
                digest.update(&buffer[..n]);
            }
            ensure!(hex::encode(digest.finalize()) == *hash, "restored blob checksum mismatch");
        }
        File::open(destination)?.sync_all()?;
        if let Some(parent) = destination.parent().filter(|p| !p.as_os_str().is_empty()) { File::open(parent)?.sync_all()?; }
        // Read back restored counts before commit; database constraints validate
        // FK/type/check invariants. Blob bytes were authenticated before writing.
        for table in TABLES {
            // Audited: fixed TABLES allowlist.
            let count: i64 = sqlx::query_scalar(AssertSqlSafe(format!("SELECT count(*) FROM {table}"))).fetch_one(&mut *tx).await?;
            ensure!(u64::try_from(count)? == manifest.tables[*table].rows, "restored row count mismatch");
        }
        sqlx::query("DELETE FROM instance_reset_intents").execute(&mut *tx).await?;
        sqlx::query("DELETE FROM project_transfer_routes").execute(&mut *tx).await?;
        sqlx::query("UPDATE project_transfer_jobs SET state='unavailable',key_blob=NULL,lease_owner=NULL,lease_until=NULL,artifact_id=NULL,error_code='instance_restored',finished_at=now()")
            .execute(&mut *tx).await?;
        sqlx::query("UPDATE project_transfers SET state='cancelled',version=version+1,updated_at=now()")
            .execute(&mut *tx).await?;
        sqlx::query("UPDATE projects SET migration_fence=NULL").execute(&mut *tx).await?;
        sqlx::query("UPDATE instance_settings SET reset_pending=NULL WHERE singleton").execute(&mut *tx).await?;
        sqlx::query("UPDATE instance_settings SET id=$1,generation=$2,operating_mode='restore_validation',restored_from=$3,restored_source_id=$4,restored_at=now(),last_check=NULL,checked_at=NULL WHERE singleton")
            .bind(target_id).bind(generation).bind(manifest.header.archive_id).bind(manifest.header.instance_id).execute(&mut *tx).await?;
        sqlx::query("UPDATE instance_operations SET state=CASE WHEN state='ready' THEN 'unavailable' ELSE 'cancelled' END,key_blob=NULL,lease_owner=NULL,lease_until=NULL,artifact_id=NULL,error_code='instance_restored',finished_at=now()")
            .execute(&mut *tx).await?;
        sqlx::query("UPDATE jobs SET state='cancelled',error_code='instance_restored',lease_owner=NULL,lease_until=NULL,finished_at=now() WHERE state IN ('queued','running','retry_wait')")
            .execute(&mut *tx).await?;
        sqlx::query("UPDATE web_checks SET enabled=false,configuration=jsonb_set(configuration,'{enabled}','false'::jsonb),version=version+1").execute(&mut *tx).await?;
        sqlx::query("UPDATE web_check_runs SET status='cancelled',lease_id=NULL,lease_until=NULL,finished_at=now() WHERE status IN ('queued','running')").execute(&mut *tx).await?;
        sqlx::query("UPDATE monitors SET enabled=false,generation=$1,version=version+1,lease_owner=NULL,lease_until=NULL,updated_at=now()")
            .bind(generation).execute(&mut *tx).await?;
        sqlx::query("UPDATE push_sends SET status='failed',provider_outcome=CASE WHEN lease_owner IS NULL THEN 'cancelled' ELSE 'uncertain' END,error='instance_restored',lease_owner=NULL,lease_until=NULL WHERE status='queued'")
            .execute(&mut *tx).await?;
        sqlx::query("UPDATE notification_deliveries SET state=CASE WHEN state='running' THEN 'uncertain' ELSE 'cancelled' END,error_code='instance_restored',lease_owner=NULL,lease_until=NULL,finished_at=now() WHERE state IN ('queued','running','retry_wait')")
            .execute(&mut *tx).await?;
        Ok::<(),anyhow::Error>(())
    }.await;
    if let Err(error) = result {
        tx.rollback().await?;
        std::fs::remove_dir_all(destination)
            .context("remove this operation's uncommitted blob directory")?;
        return Err(error);
    }
    // A commit transport failure can be ambiguous. Preserve blobs in that case
    // so a committed database can never be left pointing to deleted content.
    tx.commit().await.context(
        "restore commit outcome unknown; preserve blobs and inspect target before retrying",
    )?;
    Ok(RestoreReport {
        instance_id: target_id,
        generation,
        archive_id: manifest.header.archive_id,
        source_instance_id: manifest.header.instance_id,
    })
}

/// Resume only after the operator has stopped the old instance and inspected
/// this recovered target. A running server prevents this transition so worker
/// startup and the durable state cannot disagree.
pub async fn resume(
    pool: &PgPool,
    instance: uuid::Uuid,
    generation: i64,
    archive: uuid::Uuid,
) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    let offline: bool = sqlx::query_scalar("SELECT pg_try_advisory_xact_lock($1)")
        .bind(crate::INSTANCE_SERVER_LOCK)
        .fetch_one(&mut *tx)
        .await?;
    ensure!(
        offline,
        "stop the restored server before resuming automatic workers"
    );
    crate::reset::ensure_available(pool).await?;
    let changed=sqlx::query("UPDATE instance_settings SET operating_mode='normal',version=version+1,updated_at=now() WHERE singleton AND id=$1 AND generation=$2 AND restored_from=$3 AND operating_mode='restore_validation'")
        .bind(instance).bind(generation).bind(archive).execute(&mut *tx).await?.rows_affected();
    ensure!(
        changed == 1,
        "recovered instance identity, generation or archive does not match"
    );
    // Test events accepted during recovery validation are evidence for the
    // operator, not an authorization to send their queued notifications later.
    sqlx::query("UPDATE push_sends SET status='failed',provider_outcome='cancelled',error='recovery_validation_ended',lease_owner=NULL,lease_until=NULL WHERE status='queued'")
        .execute(&mut *tx).await?;
    // The router was paused too: closing existing deliveries alone would let
    // previously unrouted validation events create new deliveries on restart.
    sqlx::query("UPDATE notification_events SET routed_at=now() WHERE routed_at IS NULL")
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE notification_deliveries SET state='cancelled',error_code='recovery_validation_ended',lease_owner=NULL,lease_until=NULL,finished_at=now() WHERE state IN ('queued','running','retry_wait')")
        .execute(&mut *tx).await?;
    sqlx::query("INSERT INTO audit_logs(id,action,target_type,target_id,payload) VALUES($1,'instance.recovery_resumed','instance',$2,$3)")
        .bind(uuid::Uuid::new_v4()).bind(instance).bind(serde_json::json!({"archiveId":archive,"generation":generation,"operator":"offline-cli"})).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}
