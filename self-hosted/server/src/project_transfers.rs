//! Durable project migration jobs. Import data and its completion receipt commit
//! together; a lost worker can never publish using a replacement worker's lease.
use crate::{instance_operations, state::AppState};
use anyhow::{Context, ensure};
use sentori_snapshot::{
    archive::{TransferBinding, TransferPhase},
    capture::CaptureControl,
    project_import::PreparedImport,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{Connection, Postgres, Transaction};
use std::{
    path::PathBuf,
    sync::{Arc, atomic::Ordering},
    time::Duration,
};
use uuid::Uuid;

pub fn directories() -> Option<(PathBuf, PathBuf)> {
    let value = std::env::var("SENTORI_ATTACHMENT_STORE").ok()?;
    let blob = PathBuf::from(value.strip_prefix("fs:")?);
    // Cloud mounts only the blob volume writable. A non-CAS subdirectory is
    // durable under the same mount and is ignored by the two-hex-prefix blob GC.
    (blob.is_absolute() && blob.file_name().is_some())
        .then(|| (blob.clone(), blob.join(".transfers")))
}
pub fn configured(state: &AppState) -> bool {
    state.vault.is_some() && directories().is_some()
}
pub fn prepare_directory() -> anyhow::Result<PathBuf> {
    let (_, directory) = directories().context("transfer storage unavailable")?;
    std::fs::create_dir_all(&directory)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(directory)
}
pub async fn audit(
    tx: &mut Transaction<'_, Postgres>,
    actor: Option<Uuid>,
    id: Uuid,
    action: &str,
    payload: Value,
) -> anyhow::Result<()> {
    sqlx::query("INSERT INTO audit_logs(id,actor_user_id,action,target_type,target_id,payload,ip) VALUES($1,$2,$3,'project_transfer',$4,$5,$6)")
        .bind(Uuid::now_v7()).bind(actor).bind(action).bind(id.to_string()).bind(payload).bind(sentori_request_origin::current()).execute(&mut **tx).await?;
    Ok(())
}
#[derive(sqlx::FromRow, Clone)]
struct Work {
    id: Uuid,
    transfer_id: Uuid,
    kind: String,
    phase: String,
    key_blob: Vec<u8>,
    attempt: i32,
    artifact_id: Option<Uuid>,
}
#[derive(sqlx::FromRow)]
pub struct Transfer {
    pub id: Uuid,
    pub direction: String,
    pub instance_id: Uuid,
    pub generation: i64,
    pub source_instance_id: Uuid,
    pub source_generation: i64,
    pub target_instance_id: Uuid,
    pub target_generation: i64,
    pub project_ids: Vec<Uuid>,
    pub state: String,
}
pub async fn locked(tx: &mut Transaction<'_, Postgres>, id: Uuid) -> anyhow::Result<Transfer> {
    Ok(sqlx::query_as("SELECT id,direction,instance_id,generation,source_instance_id,source_generation,target_instance_id,target_generation,project_ids,state FROM project_transfers WHERE id=$1 FOR UPDATE")
        .bind(id).fetch_one(&mut **tx).await?)
}
pub fn spawn(state: Arc<AppState>) {
    tokio::spawn(async move {
        loop {
            if configured(&state)
                && let Err(error) = tick(&state).await
            {
                tracing::warn!(%error,"project transfer worker failed");
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });
}
async fn terminal(state: &AppState, work: &Work, lease: Uuid, code: &str) -> anyhow::Result<()> {
    let mut tx = state.pool.begin().await?;
    let changed=sqlx::query("UPDATE project_transfer_jobs SET state=CASE WHEN cancel_requested THEN 'cancelled' ELSE 'failed' END,key_blob=NULL,lease_owner=NULL,lease_until=NULL,error_code=$3,finished_at=now() WHERE id=$1 AND lease_owner=$2 AND state='running'")
        .bind(work.id).bind(lease).bind(code).execute(&mut *tx).await?.rows_affected();
    if changed == 1 {
        audit(
            &mut tx,
            None,
            work.transfer_id,
            "transfer.job.failed",
            json!({"jobId":work.id,"code":code}),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(())
}
async fn tick(state: &Arc<AppState>) -> anyhow::Result<()> {
    let mut guard = state.pool.acquire().await?.detach();
    let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
        .bind(0x5345_4e54_5452_4e53_i64)
        .fetch_one(&mut guard)
        .await?;
    if !locked {
        return Ok(());
    }
    let lease = Uuid::now_v7();
    let work:Option<Work>=sqlx::query_as("UPDATE project_transfer_jobs SET state='running',lease_owner=$1,lease_until=now()+interval '45 seconds',attempt=attempt+1,started_at=coalesce(started_at,now()) WHERE id=(SELECT id FROM project_transfer_jobs WHERE state='queued' OR (state='running' AND lease_until<now()) ORDER BY created_at LIMIT 1 FOR UPDATE SKIP LOCKED) RETURNING id,transfer_id,kind,phase,key_blob,attempt,artifact_id")
        .bind(lease).fetch_optional(&state.pool).await?;
    let Some(work) = work else {
        return Ok(());
    };
    if work.attempt > 3 {
        terminal(state, &work, lease, "retry_exhausted").await?;
        return Ok(());
    }
    let key = instance_operations::open_key(state, work.id, &work.key_blob);
    let Ok(key) = key else {
        terminal(state, &work, lease, "key_unavailable").await?;
        return Ok(());
    };
    let directory = prepare_directory()?;
    let artifact = if work.kind == "import" {
        work.artifact_id.context("upload missing")?
    } else {
        lease
    };
    let path = directory.join(format!("{artifact}.sentori"));
    let control = Arc::new(CaptureControl::default());
    let worker_control = control.clone();
    let worker_state = state.clone();
    let worker_work = work.clone();
    let worker_path = path.clone();
    let runtime = tokio::runtime::Handle::current();
    let mut task = tokio::task::spawn_blocking(move || {
        runtime.block_on(attempt(
            &worker_state,
            &worker_work,
            lease,
            artifact,
            &worker_path,
            key,
            &worker_control,
        ))
    });
    let deadline = std::time::Instant::now() + Duration::from_hours(6);
    let result = loop {
        tokio::select! {
            result=&mut task => break result.context("transfer worker join failed")?,
            ()=tokio::time::sleep(Duration::from_secs(1)) => {
                let progress=json!({"rows":control.rows.load(Ordering::Relaxed),"blobBytes":control.blob_bytes.load(Ordering::Relaxed),"phase":if control.verifying.load(Ordering::Relaxed){"verifying"}else{"copying"}});
                let heartbeat:Result<Option<bool>,sqlx::Error>=sqlx::query_scalar("UPDATE project_transfer_jobs SET lease_until=now()+interval '45 seconds',progress=$3 WHERE id=$1 AND lease_owner=$2 AND state='running' RETURNING cancel_requested")
                    .bind(work.id).bind(lease).bind(progress).fetch_optional(&state.pool).await;
                if std::time::Instant::now() >= deadline || sqlx::query("SELECT 1").execute(&mut guard).await.is_err() || !matches!(heartbeat,Ok(Some(false))) { control.cancelled.store(true,Ordering::Relaxed); }
            }
        }
    };
    if let Err(error) = result {
        tracing::warn!(job=%work.id,%error,"project transfer attempt failed");
        terminal(state, &work, lease, "transfer_failed").await?;
        if work.kind == "export" {
            let _ = tokio::fs::remove_file(path).await;
        }
    }
    guard.close().await?;
    Ok(())
}
// Data, quota checks and lease publication form one transaction.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
async fn attempt(
    state: &AppState,
    work: &Work,
    lease: Uuid,
    artifact: Uuid,
    path: &std::path::Path,
    key: sentori_snapshot::RecoveryKey,
    control: &CaptureControl,
) -> anyhow::Result<()> {
    let (blob, _) = directories().context("transfer storage unavailable")?;
    let phase = if work.phase == "final" {
        TransferPhase::Final
    } else {
        TransferPhase::Copy
    };
    // Hold only short locks while capturing a consistent source snapshot.
    let mut tx = state.pool.begin().await?;
    let identity: (Uuid, i64, String) = sqlx::query_as(
        "SELECT id,generation,operating_mode FROM instance_settings WHERE singleton FOR SHARE",
    )
    .fetch_one(&mut *tx)
    .await?;
    let transfer = locked(&mut tx, work.transfer_id).await?;
    ensure!(
        identity == (transfer.instance_id, transfer.generation, "normal".into()),
        "instance changed"
    );
    tx.commit().await?;
    let binding = TransferBinding {
        id: transfer.id,
        target_instance_id: transfer.target_instance_id,
        target_generation: transfer.target_generation,
        phase,
    };
    let mut prepared = None;
    let manifest = if work.kind == "export" {
        ensure!(transfer.direction == "source", "wrong direction");
        sentori_snapshot::capture::capture_transfer(
            &state.pool,
            &blob,
            path,
            key,
            env!("CARGO_PKG_VERSION"),
            &binding,
            control,
        )
        .await?
    } else {
        control.verifying.store(true, Ordering::Relaxed);
        let import = PreparedImport::open(path, key, control)?;
        ensure!(
            import.manifest.header.transfer.as_ref() == Some(&binding)
                && import.manifest.header.instance_id == transfer.source_instance_id
                && import.manifest.header.generation == transfer.source_generation
                && import.manifest.header.project_ids == transfer.project_ids,
            "source/target package binding differs"
        );
        let manifest = import.manifest.clone();
        prepared = Some(import);
        manifest
    };
    let bytes = i64::try_from(std::fs::metadata(path)?.len())?;
    let mut hash = Sha256::new();
    let mut file = std::fs::File::open(path)?;
    let mut buffer = vec![0; 512 * 1024];
    loop {
        control.check()?;
        let count = std::io::Read::read(&mut file, &mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    let sha = hex::encode(hash.finalize());
    let mut tx = state.pool.begin().await?;
    let identity: (Uuid, i64, String) = sqlx::query_as(
        "SELECT id,generation,operating_mode FROM instance_settings WHERE singleton FOR SHARE",
    )
    .fetch_one(&mut *tx)
    .await?;
    // Same admission order as ordinary Cloud writes: entitlement before projects.
    if prepared.is_some() && state.cloud.is_some() {
        crate::cloud_quota::transfer_lock(&mut tx).await?;
    }
    let transfer = locked(&mut tx, work.transfer_id).await?;
    ensure!(
        identity == (transfer.instance_id, transfer.generation, "normal".into()),
        "instance changed"
    );
    let mut summary = json!({"archiveId":manifest.header.archive_id,"phase":work.phase,"sourceInstanceId":manifest.header.instance_id,"rows":manifest.tables.values().map(|t|t.rows).sum::<u64>(),"blobBytes":manifest.blob_bytes,"tables":manifest.tables});
    if let Some(mut prepared) = prepared {
        ensure!(
            transfer.direction == "target"
                && matches!(transfer.state.as_str(), "copying" | "copied"),
            "destination already finalized"
        );
        let mapping = prepared.project_mapping(transfer.id);
        let projects: Vec<Uuid> = mapping.values().copied().collect();
        sqlx::query("SELECT set_config('sentori.transfer_id',$1,true)")
            .bind(transfer.id.to_string())
            .execute(&mut *tx)
            .await?;
        let unsafe_existing:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM projects WHERE id=ANY($1) AND migration_fence IS DISTINCT FROM $2)").bind(&projects).bind(transfer.id).fetch_one(&mut *tx).await?;
        ensure!(
            !unsafe_existing,
            "destination project is not staged by this transfer"
        );
        sqlx::query("DELETE FROM projects WHERE id=ANY($1) AND migration_fence=$2")
            .bind(&projects)
            .bind(transfer.id)
            .execute(&mut *tx)
            .await?;
        let checksums = prepared.apply(&mut tx, &blob, transfer.id, control).await?;
        sqlx::query("UPDATE projects SET migration_fence=$2 WHERE id=ANY($1)")
            .bind(&projects)
            .bind(transfer.id)
            .execute(&mut *tx)
            .await?;
        if state.cloud.is_some() {
            crate::cloud_quota::transfer_check(&mut tx).await?;
        }
        summary["readback"] = serde_json::to_value(checksums)?;
        sqlx::query("UPDATE project_transfers SET project_mapping=$2 WHERE id=$1")
            .bind(transfer.id)
            .bind(serde_json::to_value(mapping)?)
            .execute(&mut *tx)
            .await?;
    } else {
        ensure!(
            transfer.direction == "source"
                && if phase == TransferPhase::Final {
                    transfer.state == "sealed"
                } else {
                    matches!(transfer.state.as_str(), "copying" | "copied")
                },
            "source phase changed"
        );
    }
    control.check()?;
    let changed=sqlx::query("UPDATE project_transfer_jobs SET state='ready',key_blob=NULL,lease_owner=NULL,lease_until=NULL,artifact_id=$3,artifact_bytes=$4,artifact_sha256=$5,manifest=$6,finished_at=now() WHERE id=$1 AND lease_owner=$2 AND state='running' AND NOT cancel_requested AND lease_until>clock_timestamp()")
        .bind(work.id).bind(lease).bind(artifact).bind(bytes).bind(&sha).bind(&summary).execute(&mut *tx).await?.rows_affected();
    ensure!(changed == 1, "worker lease lost");
    let next = if phase == TransferPhase::Final {
        if work.kind == "import" {
            "ready"
        } else {
            "sealed"
        }
    } else {
        "copied"
    };
    sqlx::query("UPDATE project_transfers SET state=$2,version=version+1,latest_archive_id=$3,latest_manifest=$4,updated_at=now() WHERE id=$1")
        .bind(transfer.id).bind(next).bind(manifest.header.archive_id).bind(&summary).execute(&mut *tx).await?;
    audit(
        &mut tx,
        None,
        transfer.id,
        "transfer.job.ready",
        json!({"jobId":work.id,"phase":work.phase,"sha256":sha}),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}
