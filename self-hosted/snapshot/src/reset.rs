//! Offline, backup-first reset. Database clearing commits with a startup fence;
//! only durable blob cleanup and an external receipt can remove that fence.
use std::{
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{AssertSqlSafe, Connection, PgConnection, PgPool};
use uuid::Uuid;

use crate::{RecoveryKey, TABLES};

/// The connection must remain alive for the entire command. Detached physical
/// connections cannot accidentally return a session lock to the pool.
pub async fn shared_guard(pool: &PgPool) -> anyhow::Result<PgConnection> {
    let mut conn = pool.acquire().await?.detach();
    let available: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock_shared($1)")
        .bind(crate::INSTANCE_SERVER_LOCK)
        .fetch_one(&mut conn)
        .await?;
    ensure!(available, "an offline instance operation is running");
    Ok(conn)
}

pub async fn ensure_available(pool: &PgPool) -> anyhow::Result<()> {
    let exists: bool =
        sqlx::query_scalar("SELECT to_regclass('public.instance_settings') IS NOT NULL")
            .fetch_one(pool)
            .await?;
    if !exists {
        return Ok(());
    }
    // to_jsonb permits this guard before migrations on an older installation.
    let pending: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM instance_settings i WHERE COALESCE(to_jsonb(i)->'reset_pending','null'::jsonb)<>'null'::jsonb)")
        .fetch_one(pool).await?;
    ensure!(
        !pending,
        "instance reset is incomplete; run sentori-operator finish-reset with its external journal before starting or modifying this instance"
    );
    Ok(())
}

pub async fn offline_guard(pool: &PgPool) -> anyhow::Result<PgConnection> {
    let mut conn = pool.acquire().await?.detach();
    let offline: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
        .bind(crate::INSTANCE_SERVER_LOCK)
        .fetch_one(&mut conn)
        .await?;
    ensure!(
        offline,
        "stop every server and operator command before the offline operation"
    );
    Ok(conn)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Journal {
    format: u8,
    operation_id: Uuid,
    instance_id: Uuid,
    instance_name: String,
    old_generation: i64,
    new_generation: i64,
    actor_user_id: Uuid,
    prepared_at: String,
    archive_id: Uuid,
    archive_sha256: String,
    archive: PathBuf,
    blob_root: PathBuf,
    journal: PathBuf,
}

pub struct ResetRequest<'a> {
    pub permit: &'a str,
    pub instance_id: Uuid,
    pub generation: i64,
    pub instance_name: &'a str,
    pub blob_root: &'a Path,
    pub archive: &'a Path,
    pub journal: &'a Path,
    pub recovery_key: RecoveryKey,
    pub master_key: Option<RecoveryKey>,
    pub application_version: &'a str,
}

fn new_path(path: &Path, blob_root: &Path) -> anyhow::Result<PathBuf> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent = parent
        .canonicalize()
        .context("independent storage directory must already exist")?;
    ensure!(
        !parent.starts_with(blob_root),
        "backup and journal must be outside the instance blob directory"
    );
    let path = parent.join(path.file_name().context("a new filename is required")?);
    ensure!(
        !path.try_exists()? && std::fs::symlink_metadata(&path).is_err(),
        "output already exists"
    );
    Ok(path)
}

fn persist_json(path: &Path, value: &serde_json::Value) -> anyhow::Result<()> {
    // Publish only a complete, synced JSON document. A crash leaves an unnamed
    // temporary file, never a truncated journal that falsely looks resumable.
    let parent = path.parent().context("journal parent required")?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer(temporary.as_file_mut(), value)?;
    temporary.write_all(b"\n")?;
    temporary.as_file().sync_all()?;
    temporary.persist_noclobber(path).map_err(|e| e.error)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

fn sha256_file(file: &mut File) -> anyhow::Result<String> {
    file.seek(SeekFrom::Start(0))?;
    let mut digest = Sha256::new();
    let mut chunk = vec![0; crate::archive::CHUNK_SIZE];
    loop {
        let n = file.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        digest.update(&chunk[..n]);
    }
    file.seek(SeekFrom::Start(0))?;
    Ok(hex::encode(digest.finalize()))
}

/// Strictly recognize the store's own fanout and files, including interrupted
/// .bin.tmp writes. Never recurse through arbitrary paths or follow symlinks.
fn blob_files(root: &Path) -> anyhow::Result<Vec<PathBuf>> {
    ensure!(
        std::fs::symlink_metadata(root)?.file_type().is_dir(),
        "blob root must be a real directory"
    );
    let mut files = Vec::new();
    for directory in std::fs::read_dir(root)? {
        let directory = directory?;
        let prefix = directory.file_name();
        let prefix = prefix.to_str().context("non-UTF8 blob directory")?;
        ensure!(
            prefix.len() == 2
                && prefix
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
                && directory.file_type()?.is_dir(),
            "unknown file or symlink in blob root; inspect it before reset"
        );
        for file in std::fs::read_dir(directory.path())? {
            let file = file?;
            let name = file.file_name();
            let name = name.to_str().context("non-UTF8 blob filename")?;
            let hash = name
                .strip_suffix(".bin")
                .or_else(|| name.strip_suffix(".bin.tmp"));
            ensure!(
                file.file_type()?.is_file()
                    && hash.is_some_and(|suffix| crate::verify::valid_hash(&format!(
                        "{prefix}{suffix}"
                    ))),
                "unknown blob file or symlink; inspect it before reset"
            );
            files.push(file.path());
        }
    }
    Ok(files)
}

pub async fn reset(pool: &PgPool, request: ResetRequest<'_>) -> anyhow::Result<Journal> {
    let mut offline = offline_guard(pool).await?;
    ensure_available(pool).await?;
    let cloud: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM cloud_instance_binding)")
        .fetch_one(pool)
        .await?;
    ensure!(!cloud, "Cloud lifecycle is managed by the control plane");
    let root = request.blob_root.canonicalize()?;
    let archive = new_path(request.archive, &root)?;
    let journal_path = new_path(request.journal, &root)?;
    ensure!(
        archive != journal_path && archive != journal_path.with_extension("complete.json"),
        "backup and journal paths must differ"
    );
    new_path(&journal_path.with_extension("complete.json"), &root)?;
    blob_files(&root)?;
    let permit_hash = hex::encode(Sha256::digest(request.permit.trim().as_bytes()));
    let permit: Option<(Uuid, Uuid, String)> = sqlx::query_as("SELECT r.id,r.actor_user_id,u.password_hash FROM instance_reset_intents r JOIN users u ON u.id=r.actor_user_id JOIN auth_sessions s ON s.id_hash=r.session_hash AND s.user_id=u.id JOIN instance_settings i ON i.singleton WHERE r.permit_hash=$1 AND r.expires_at>now() AND s.expires_at>now() AND u.role='superadmin' AND r.instance_id=$2 AND i.id=$2 AND r.generation=$3 AND i.generation=$3 AND r.instance_name=$4 AND i.name=$4 AND i.operating_mode='normal' AND r.password_fingerprint=encode(sha256(convert_to(u.password_hash,'UTF8')),'hex')")
        .bind(&permit_hash).bind(request.instance_id).bind(request.generation).bind(request.instance_name).fetch_optional(pool).await?;
    let (operation_id, actor_user_id, _) = permit
        .context("reset permit expired, revoked, or does not match the typed instance identity")?;
    let new_generation = request
        .generation
        .checked_add(1)
        .context("instance generation exhausted")?;
    let mut tx = offline.begin().await?;
    sqlx::query("SET LOCAL lock_timeout='10s'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(crate::BLOB_GC_LOCK)
        .execute(&mut *tx)
        .await?;
    // These locks remain until the clearing commit, including throughout the
    // backup. Even a writer whose lifetime lock connection failed cannot write
    // rows between the final backup snapshot and the reset transaction.
    // Audited: fixed compile-time table allowlist, never request input.
    sqlx::query(AssertSqlSafe(format!(
        "LOCK TABLE {} IN SHARE MODE",
        TABLES.join(",")
    )))
    .execute(&mut *tx)
    .await?;
    let manifest = crate::capture::capture_for_reset(
        pool,
        &root,
        &archive,
        request.recovery_key.clone(),
        request.application_version,
        request.master_key,
    )
    .await?;
    ensure!(
        manifest.header.instance_id == request.instance_id
            && manifest.header.generation == request.generation
            && manifest.header.migrations == crate::expected_migrations(),
        "backup does not match this instance and binary schema"
    );
    let journal = Journal {
        format: 1,
        operation_id,
        instance_id: request.instance_id,
        instance_name: request.instance_name.into(),
        old_generation: request.generation,
        new_generation,
        actor_user_id,
        prepared_at: time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)?,
        archive_id: manifest.header.archive_id,
        archive_sha256: sha256_file(&mut File::open(&archive)?)?,
        archive,
        blob_root: root,
        journal: journal_path,
    };
    let evidence = serde_json::to_value(&journal)?;
    // The verified archive and durable external evidence exist BEFORE rows go.
    persist_json(&journal.journal, &evidence)?;
    // Audited: compile-time table allowlist only, never user input. No CASCADE.
    sqlx::query(AssertSqlSafe(format!(
        "LOCK TABLE {} IN ACCESS EXCLUSIVE MODE",
        TABLES.join(",")
    )))
    .execute(&mut *tx)
    .await?;
    let unchanged: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM instance_settings i JOIN instance_reset_intents r ON r.instance_id=i.id JOIN users u ON u.id=r.actor_user_id JOIN auth_sessions s ON s.id_hash=r.session_hash AND s.user_id=u.id WHERE i.singleton AND i.id=$1 AND i.generation=$2 AND i.name=$3 AND i.reset_pending IS NULL AND i.operating_mode='normal' AND r.id=$4 AND r.permit_hash=$5 AND u.role='superadmin' AND s.expires_at>now() AND r.password_fingerprint=encode(sha256(convert_to(u.password_hash,'UTF8')),'hex'))")
        .bind(journal.instance_id).bind(journal.old_generation).bind(&journal.instance_name).bind(journal.operation_id).bind(permit_hash).fetch_one(&mut *tx).await?;
    ensure!(
        unchanged,
        "instance or reset authorization changed during backup; nothing was cleared"
    );
    sqlx::query("SET LOCAL sentori.offline_recovery='on'")
        .execute(&mut *tx)
        .await?;
    for table in TABLES.iter().rev().filter(|t| **t != "instance_settings") {
        // Audited: fixed TABLES allowlist, reverse FK order. Unknown referencing
        // tables cause a rollback rather than cascading into unrelated data.
        sqlx::query(AssertSqlSafe(format!("DELETE FROM {table}")))
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query("UPDATE instance_settings SET generation=$1,version=version+1,initialized_at=NULL,last_check=NULL,checked_at=NULL,restored_from=NULL,restored_source_id=NULL,restored_at=NULL,reset_pending=$2,updated_at=now() WHERE singleton")
        .bind(new_generation).bind(&evidence).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO audit_logs(id,action,target_type,target_id,payload) VALUES($1,'instance.reset.cleared','instance',$2,$3)")
        .bind(Uuid::new_v4()).bind(journal.instance_id.to_string()).bind(&evidence).execute(&mut *tx).await?;
    tx.commit().await.context(
        "reset commit outcome unknown; preserve external backup and journal, then run finish-reset",
    )?;
    finish_locked(&mut offline, &journal, request.recovery_key)
        .await
        .context("reset remains fenced; preserve backup and journal and retry finish-reset")?;
    Ok(journal)
}

pub async fn finish(pool: &PgPool, path: &Path, key: RecoveryKey) -> anyhow::Result<Journal> {
    let mut offline = offline_guard(pool).await?;
    let path = path.canonicalize()?;
    let file = File::open(&path)?;
    ensure!(
        file.metadata()?.len() <= 64 * 1024,
        "reset journal too large"
    );
    let journal: Journal = serde_json::from_reader(BufReader::new(file))?;
    ensure!(
        journal.format == 1 && journal.journal == path,
        "reset journal path or version differs"
    );
    finish_locked(&mut offline, &journal, key).await?;
    Ok(journal)
}

async fn finish_locked(
    offline: &mut PgConnection,
    journal: &Journal,
    key: RecoveryKey,
) -> anyhow::Result<()> {
    let evidence = serde_json::to_value(journal)?;
    let (id, generation, pending): (Uuid, i64, Option<serde_json::Value>) =
        sqlx::query_as("SELECT id,generation,reset_pending FROM instance_settings WHERE singleton")
            .fetch_one(&mut *offline)
            .await?;
    ensure!(
        id == journal.instance_id && generation == journal.new_generation,
        "journal is for a different instance or generation"
    );
    let receipt = serde_json::json!({"status":"complete","reset":evidence});
    let receipt_path = journal.journal.with_extension("complete.json");
    if pending.is_none() {
        let completed: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM audit_logs WHERE action='instance.reset.completed' AND payload=$1)")
            .bind(&evidence).fetch_one(&mut *offline).await?;
        ensure!(completed, "reset has no matching completion evidence");
        ensure!(
            serde_json::from_reader::<_, serde_json::Value>(File::open(receipt_path)?)? == receipt,
            "external completion receipt differs"
        );
        return Ok(());
    }
    ensure!(
        pending.as_ref() == Some(&evidence),
        "journal differs from durable pending reset"
    );
    ensure!(
        journal.blob_root.canonicalize()? == journal.blob_root
            && journal.archive.canonicalize()? == journal.archive,
        "reset paths changed"
    );
    let mut archive = File::open(&journal.archive)?;
    ensure!(
        sha256_file(&mut archive)? == journal.archive_sha256,
        "reset backup ciphertext changed"
    );
    let manifest = crate::verify::verify(BufReader::new(archive), key)?;
    ensure!(
        manifest.header.archive_id == journal.archive_id
            && manifest.header.instance_id == journal.instance_id
            && manifest.header.generation == journal.old_generation
            && manifest.header.mode == crate::archive::Mode::DisasterRecovery,
        "reset recovery archive differs"
    );
    // Preflight the WHOLE tree before removing any path. On I/O failure the
    // pending database fence survives and another run can continue safely.
    for path in blob_files(&journal.blob_root)? {
        std::fs::remove_file(path)?;
    }
    for entry in std::fs::read_dir(&journal.blob_root)? {
        let path = entry?.path();
        File::open(&path)?.sync_all()?;
        std::fs::remove_dir(path)?;
    }
    File::open(&journal.blob_root)?.sync_all()?;
    if receipt_path.try_exists()? {
        ensure!(
            serde_json::from_reader::<_, serde_json::Value>(File::open(&receipt_path)?)? == receipt,
            "external completion receipt differs"
        );
    } else {
        persist_json(&receipt_path, &receipt)?;
    }
    let mut tx = offline.begin().await?;
    let changed = sqlx::query("UPDATE instance_settings SET reset_pending=NULL,updated_at=now() WHERE singleton AND id=$1 AND generation=$2 AND reset_pending=$3")
        .bind(journal.instance_id).bind(journal.new_generation).bind(&evidence).execute(&mut *tx).await?.rows_affected();
    ensure!(changed == 1, "pending reset changed");
    sqlx::query("INSERT INTO audit_logs(id,action,target_type,target_id,payload) VALUES($1,'instance.reset.completed','instance',$2,$3)")
        .bind(Uuid::new_v4()).bind(journal.instance_id.to_string()).bind(evidence).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_rejects_unknown_content_before_deleting_anything() -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        std::fs::create_dir(directory.path().join("ab"))?;
        let blob = directory
            .path()
            .join("ab")
            .join(format!("{}.bin", "c".repeat(62)));
        std::fs::write(&blob, b"old blob")?;
        std::fs::write(directory.path().join("important.txt"), b"unrelated")?;
        assert!(blob_files(directory.path()).is_err());
        assert!(blob.exists());
        std::fs::remove_file(directory.path().join("important.txt"))?;
        assert_eq!(blob_files(directory.path())?, vec![blob]);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_never_follows_symlinks() -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let outside = tempfile::tempdir()?;
        std::os::unix::fs::symlink(outside.path(), directory.path().join("ab"))?;
        assert!(blob_files(directory.path()).is_err());
        Ok(())
    }
}
