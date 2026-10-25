//! Restartable, fenced lifecycle work. Every replica shares a PostgreSQL session
//! lock; each attempt writes to a unique file and publishes only via its lease.
use crate::state::AppState;
use anyhow::{Context, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{Connection, PgPool};
use std::{
    path::PathBuf,
    sync::{Arc, atomic::Ordering},
    time::Duration,
};
use uuid::Uuid;

const WORK_LOCK: i64 = 0x5345_4e54_4241_434b;

pub fn directories() -> Option<(PathBuf, PathBuf)> {
    let blob = std::env::var("SENTORI_ATTACHMENT_STORE").ok()?;
    let blob = PathBuf::from(blob.strip_prefix("fs:")?);
    let backups = PathBuf::from(std::env::var("SENTORI_BACKUP_DIR").ok()?);
    (blob.is_absolute() && backups.is_absolute() && blob != backups).then_some((blob, backups))
}

pub fn configured(state: &AppState) -> bool {
    state.vault.is_some() && directories().is_some()
}

pub fn seal_key(state: &AppState, id: Uuid, key: &str) -> anyhow::Result<Vec<u8>> {
    Ok(state
        .vault
        .as_ref()
        .context("vault unavailable")?
        .seal(&serde_json::to_vec(&json!({"operation":id,"key":key}))?)?)
}

pub(crate) fn open_key(
    state: &AppState,
    id: Uuid,
    blob: &[u8],
) -> anyhow::Result<sentori_snapshot::RecoveryKey> {
    let value: Value = serde_json::from_slice(
        &state
            .vault
            .as_ref()
            .context("vault unavailable")?
            .open(blob)?,
    )?;
    ensure!(
        value["operation"] == id.to_string(),
        "operation key binding mismatch"
    );
    sentori_snapshot::parse_key(value["key"].as_str().context("missing recovery key")?)
}

pub fn spawn(state: Arc<AppState>) {
    tokio::spawn(async move {
        loop {
            if configured(&state)
                && let Err(error) = tick(&state).await
            {
                tracing::warn!(%error,"instance backup worker failed");
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });
}

#[derive(sqlx::FromRow)]
struct Work {
    id: Uuid,
    key_blob: Vec<u8>,
    instance_id: Uuid,
    generation: i64,
    attempt: i32,
    kind: String,
    artifact_id: Option<Uuid>,
}

async fn terminal(
    pool: &PgPool,
    id: Uuid,
    lease: Uuid,
    state: &str,
    code: &str,
) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    let changed=sqlx::query("UPDATE instance_operations SET state=$3,error_code=$4,key_blob=NULL,artifact_id=NULL,artifact_bytes=NULL,artifact_sha256=NULL,lease_owner=NULL,lease_until=NULL,finished_at=now() WHERE id=$1 AND lease_owner=$2 AND state='running'")
        .bind(id).bind(lease).bind(state).bind(code).execute(&mut *tx).await?.rows_affected();
    if changed == 1 {
        sqlx::query("INSERT INTO audit_logs (id,action,target_type,target_id,payload,ip) VALUES ($1,'instance.backup.finished','instance_operation',$2,$3,$4)")
            .bind(Uuid::now_v7()).bind(id.to_string()).bind(json!({"state":state,"errorCode":code})).bind(sentori_request_origin::current()).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}

async fn tick(state: &Arc<AppState>) -> anyhow::Result<()> {
    let mut guard = state.pool.acquire().await?.detach();
    let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
        .bind(WORK_LOCK)
        .fetch_one(&mut guard)
        .await?;
    if !locked {
        return Ok(());
    }
    let lease = Uuid::now_v7();
    let work: Option<Work> = sqlx::query_as("UPDATE instance_operations SET state='running',lease_owner=$1,lease_until=now()+interval '45 seconds',started_at=COALESCE(started_at,now()),attempt=attempt+1 WHERE id=(SELECT id FROM instance_operations WHERE state='queued' OR (state='running' AND lease_until<now()) ORDER BY created_at LIMIT 1 FOR UPDATE SKIP LOCKED) RETURNING id,key_blob,instance_id,generation,attempt,kind,artifact_id")
        .bind(lease).fetch_optional(&state.pool).await?;
    let Some(work) = work else {
        return Ok(());
    };
    if work.attempt > 3 {
        terminal(&state.pool, work.id, lease, "failed", "retry_exhausted").await?;
        return Ok(());
    }
    let identity: (Uuid, i64, String) = sqlx::query_as(
        "SELECT id,generation,operating_mode FROM instance_settings WHERE singleton",
    )
    .fetch_one(&state.pool)
    .await?;
    if identity != (work.instance_id, work.generation, "normal".into()) {
        terminal(&state.pool, work.id, lease, "cancelled", "instance_changed").await?;
        return Ok(());
    }
    let Ok(key) = open_key(state, work.id, &work.key_blob) else {
        terminal(&state.pool, work.id, lease, "failed", "key_unavailable").await?;
        return Ok(());
    };
    let (_, directory) = directories().context("backup storage unavailable")?;
    let artifact_id = if work.kind == "verify" {
        work.artifact_id.context("missing uploaded archive")?
    } else {
        lease
    };
    let path = directory.join(format!("{artifact_id}.sentori"));
    let control = Arc::new(sentori_snapshot::capture::CaptureControl::default());
    let mut task = capture_attempt(
        state,
        path.clone(),
        key,
        control.clone(),
        work.kind == "verify",
    )?;
    let result = loop {
        tokio::select! {
            result=&mut task=>break result.context("backup worker join failed")?,
            ()=tokio::time::sleep(Duration::from_secs(1))=> {
                let progress=json!({"rows":control.rows.load(Ordering::Relaxed),"blobBytes":control.blob_bytes.load(Ordering::Relaxed),"phase":if control.verifying.load(Ordering::Relaxed){"verifying"}else{"capturing"}});
                let heartbeat: Result<Option<bool>, sqlx::Error>=sqlx::query_scalar("UPDATE instance_operations SET lease_until=now()+interval '45 seconds',progress=$3 WHERE id=$1 AND lease_owner=$2 AND state='running' RETURNING cancel_requested")
                    .bind(work.id).bind(lease).bind(progress).fetch_optional(&state.pool).await;
                let alive=sqlx::query("SELECT 1").execute(&mut guard).await.is_ok();
                if !alive || !matches!(heartbeat,Ok(Some(false))) { control.cancelled.store(true,Ordering::Relaxed); }
            }
        }
    };
    match result {
        Ok((manifest, bytes, sha)) => {
            let mut tx = state.pool.begin().await?;
            let changed=sqlx::query("UPDATE instance_operations SET state='ready',key_blob=NULL,lease_owner=NULL,lease_until=NULL,artifact_id=$3,artifact_bytes=$4,artifact_sha256=$5,manifest=$6,finished_at=now() WHERE id=$1 AND lease_owner=$2 AND state='running' AND NOT cancel_requested AND EXISTS(SELECT 1 FROM instance_settings WHERE singleton AND id=instance_operations.instance_id AND generation=instance_operations.generation AND operating_mode='normal')")
                .bind(work.id).bind(lease).bind(artifact_id).bind(bytes).bind(&sha).bind(manifest).execute(&mut *tx).await?.rows_affected();
            if changed == 1 {
                sqlx::query("INSERT INTO audit_logs (id,action,target_type,target_id,payload,ip) VALUES ($1,'instance.backup.ready','instance_operation',$2,$3,$4)")
                    .bind(Uuid::now_v7()).bind(work.id.to_string()).bind(json!({"bytes":bytes,"sha256":sha})).bind(sentori_request_origin::current()).execute(&mut *tx).await?;
            }
            tx.commit().await?;
            if changed == 0 {
                let _ = tokio::fs::remove_file(&path).await;
                terminal(&state.pool, work.id, lease, "cancelled", "cancelled").await?;
            }
        }
        Err(error) => {
            // Never include the key, row data or filesystem paths in API error payloads.
            tracing::warn!(operation=%work.id,%error,"backup capture failed");
            let _ = tokio::fs::remove_file(&path).await;
            let cancelled = control.cancelled.load(Ordering::Relaxed);
            terminal(
                &state.pool,
                work.id,
                lease,
                if cancelled { "cancelled" } else { "failed" },
                if cancelled {
                    "cancelled"
                } else if work.kind == "verify" {
                    "verification_failed"
                } else {
                    "capture_failed"
                },
            )
            .await?;
        }
    }
    guard.close().await?;
    Ok(())
}

type Artifact = (Value, i64, String);
fn capture_attempt(
    state: &AppState,
    path: PathBuf,
    key: sentori_snapshot::RecoveryKey,
    control: Arc<sentori_snapshot::capture::CaptureControl>,
    verify_only: bool,
) -> anyhow::Result<tokio::task::JoinHandle<anyhow::Result<Artifact>>> {
    let worker_path = path;
    let (blob, directory) = directories().context("backup storage unavailable")?;
    let pool = state.pool.clone();
    let master = if verify_only {
        None
    } else {
        Some(sentori_snapshot::parse_key(
            &crate::env_config::env_or_file("SENTORI_MASTER_KEY")
                .context("instance master key unavailable")?,
        )?)
    };
    let worker_control = control;
    let runtime = tokio::runtime::Handle::current();
    Ok(tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
        std::fs::create_dir_all(&directory)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))?;
        }
        let manifest = if verify_only {
            worker_control.verifying.store(true, Ordering::Relaxed);
            sentori_snapshot::verify::verify_controlled(
                std::io::BufReader::new(std::fs::File::open(&worker_path)?),
                key,
                &worker_control,
            )?
        } else {
            runtime.block_on(async {
                tokio::time::timeout(
                    Duration::from_hours(6),
                    sentori_snapshot::capture::capture_controlled(
                        &pool,
                        &blob,
                        &worker_path,
                        key,
                        env!("CARGO_PKG_VERSION"),
                        master,
                        &worker_control,
                    ),
                )
                .await?
            })?
        };
        anyhow::ensure!(
            manifest.header.mode == sentori_snapshot::archive::Mode::DisasterRecovery,
            "instance backup verification requires a recovery archive"
        );
        let mut file = std::fs::File::open(&worker_path)?;
        let bytes = file.metadata()?.len();
        let mut hash = Sha256::new();
        let mut buffer = vec![0u8; 512 * 1024];
        loop {
            worker_control.check()?;
            let n = std::io::Read::read(&mut file, &mut buffer)?;
            if n == 0 {
                break;
            }
            hash.update(&buffer[..n]);
        }
        // Public progress contains totals, not tables/rows/credentials from the snapshot.
        let summary = json!({"archiveId":manifest.header.archive_id,"sourceInstanceId":manifest.header.instance_id,
            "generation":manifest.header.generation,"createdAt":manifest.header.created_at,
            "tables":manifest.tables.len(),"rows":manifest.tables.values().map(|t|t.rows).sum::<u64>(),
            "blobs":manifest.blobs.len(),"blobBytes":manifest.blob_bytes});
        Ok((summary, i64::try_from(bytes)?, hex::encode(hash.finalize())))
    }))
}
