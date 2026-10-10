//! Durable command queue. One bounded worker per process; project work is serialized
//! across processes. Every payload write and checkpoint is fenced by the job lease
//! and instance generation in the same database transaction.
use crate::state::AppState;
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Row, Transaction};
use std::{sync::Arc, time::Duration};
use time::OffsetDateTime;
use uuid::Uuid;

pub type Tx<'a> = Transaction<'a, Postgres>;

#[derive(Debug)]
pub enum EnqueueError {
    Database(sqlx::Error),
    Conflict,
    Full,
}
impl From<sqlx::Error> for EnqueueError {
    fn from(value: sqlx::Error) -> Self {
        Self::Database(value)
    }
}

/// Caller also commits the domain mutation (e.g. artifact replacement) in this tx.
/// The project row serializes key creation and the per-project queue bound.
pub async fn enqueue(
    tx: &mut Tx<'_>,
    project: Uuid,
    release: Uuid,
    actor: Option<Uuid>,
    key: &str,
) -> Result<Uuid, EnqueueError> {
    let generation: i64 =
        sqlx::query_scalar("SELECT generation FROM instance_settings WHERE singleton FOR SHARE")
            .fetch_one(&mut **tx)
            .await?;
    sqlx::query("SELECT id FROM projects WHERE id=$1 FOR NO KEY UPDATE")
        .bind(project)
        .fetch_one(&mut **tx)
        .await?;
    let subject = actor.map_or_else(|| "artifact-upload".to_owned(), |id| id.to_string());
    let hash = hex::encode(Sha256::digest(key.as_bytes()));
    let digest = hex::encode(Sha256::digest(
        format!("resymbolicate:{release}:{generation}").as_bytes(),
    ));
    if let Some(row) = sqlx::query("SELECT request_digest,job_id FROM idempotency_requests WHERE project_id=$1 AND subject=$2 AND key_hash=$3")
        .bind(project).bind(&subject).bind(&hash).fetch_optional(&mut **tx).await? {
        return if row.get::<String,_>("request_digest") == digest { Ok(row.get("job_id")) } else { Err(EnqueueError::Conflict) };
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs WHERE project_id=$1 AND state IN ('queued','running','retry_wait')")
        .bind(project).fetch_one(&mut **tx).await?;
    if count >= 256 {
        return Err(EnqueueError::Full);
    }
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO jobs(id,project_id,release_id,actor_user_id,generation,kind) VALUES($1,$2,$3,$4,$5,'resymbolicate')")
        .bind(id).bind(project).bind(release).bind(actor).bind(generation).execute(&mut **tx).await?;
    sqlx::query("INSERT INTO idempotency_requests(project_id,subject,key_hash,request_digest,job_id) VALUES($1,$2,$3,$4,$5)")
        .bind(project).bind(subject).bind(hash).bind(digest).bind(id).execute(&mut **tx).await?;
    sqlx::query("INSERT INTO outbox_events(id,job_id,event_type) VALUES($1,$2,'job.ready')")
        .bind(Uuid::now_v7())
        .bind(id)
        .execute(&mut **tx)
        .await?;
    sqlx::query("INSERT INTO audit_logs(id,project_id,actor_user_id,action,target_type,target_id,payload) VALUES($1,$2,$3,'job.queued','job',$4,$5)")
        .bind(Uuid::now_v7()).bind(project).bind(actor).bind(id.to_string()).bind(json!({"kind":"resymbolicate","releaseId":release})).execute(&mut **tx).await?;
    Ok(id)
}

pub fn spawn(state: Arc<AppState>) {
    tokio::spawn(async move {
        loop {
            match claim(&state.pool).await {
                Ok(Some((id, owner))) => loop {
                    match tokio::time::timeout(Duration::from_secs(45), batch(&state, id, owner))
                        .await
                    {
                        Ok(Ok(true)) => {}
                        Ok(Ok(false)) => break,
                        result => {
                            tracing::warn!(%id, error=?result, "background job batch failed");
                            let code = if result.is_err() {
                                "batch_timeout"
                            } else {
                                "processing_failed"
                            };
                            if let Err(error) = fail(&state.pool, id, owner, code).await {
                                tracing::warn!(%id, %error, "job retry persistence failed; lease will recover");
                            }
                            break;
                        }
                    }
                },
                Ok(None) => tokio::time::sleep(Duration::from_secs(1)).await,
                Err(error) => {
                    tracing::warn!(%error, "job claim failed");
                    tokio::time::sleep(Duration::from_secs(3)).await;
                }
            }
        }
    });
}

async fn claim(pool: &PgPool) -> Result<Option<(Uuid, Uuid)>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    // Short transaction only; avoids two workers claiming different jobs for the
    // same project from concurrent snapshots. No process-wide worker lock.
    let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(73636824005)")
        .fetch_one(&mut *tx)
        .await?;
    if !locked {
        return Ok(None);
    }
    let generation: i64 =
        sqlx::query_scalar("SELECT generation FROM instance_settings WHERE singleton FOR SHARE")
            .fetch_one(&mut *tx)
            .await?;
    sqlx::query("UPDATE jobs SET state=CASE WHEN cancel_requested THEN 'cancelled' ELSE 'failed' END, error_code=CASE WHEN generation<>$1 THEN 'generation_changed' WHEN cancel_requested THEN NULL ELSE 'attempts_exhausted' END,lease_owner=NULL,lease_until=NULL,finished_at=now(),updated_at=now() WHERE state IN ('queued','running','retry_wait') AND (state<>'running' OR lease_until<=now()) AND (generation<>$1 OR cancel_requested OR attempt>=max_attempts)")
        .bind(generation).execute(&mut *tx).await?;
    let id: Option<Uuid> = sqlx::query_scalar("SELECT j.id FROM jobs j JOIN outbox_events o ON o.job_id=j.id WHERE j.generation=$1 AND j.attempt<j.max_attempts AND NOT j.cancel_requested AND ((j.state IN ('queued','retry_wait') AND j.next_attempt_at<=now()) OR (j.state='running' AND j.lease_until<=now())) AND NOT EXISTS(SELECT 1 FROM jobs busy WHERE busy.project_id=j.project_id AND busy.state='running' AND busy.id<>j.id) ORDER BY j.next_attempt_at,j.created_at,j.id LIMIT 1 FOR UPDATE OF j SKIP LOCKED")
        .bind(generation).fetch_optional(&mut *tx).await?;
    let Some(id) = id else {
        tx.commit().await?;
        return Ok(None);
    };
    let owner = Uuid::now_v7();
    sqlx::query("UPDATE jobs SET state='running',attempt=attempt+1,lease_owner=$2,lease_until=now()+interval '120 seconds',updated_at=now(),error_code=NULL WHERE id=$1")
        .bind(id).bind(owner).execute(&mut *tx).await?;
    sqlx::query(
        "UPDATE outbox_events SET delivered_at=COALESCE(delivered_at,now()) WHERE job_id=$1",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Some((id, owner)))
}

async fn fail(pool: &PgPool, id: Uuid, owner: Uuid, code: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE jobs SET state=CASE WHEN cancel_requested THEN 'cancelled' WHEN attempt>=max_attempts THEN 'failed' ELSE 'retry_wait' END,error_code=$3,next_attempt_at=now()+make_interval(secs=>LEAST(300,5*attempt*attempt)),finished_at=CASE WHEN cancel_requested OR attempt>=max_attempts THEN now() ELSE NULL END,lease_owner=NULL,lease_until=NULL,updated_at=now() WHERE id=$1 AND lease_owner=$2 AND state='running' AND lease_until>now()")
        .bind(id).bind(owner).bind(code).execute(pool).await?;
    Ok(())
}

/// A batch holds at most 25 payloads; checkpoint and rewritten events commit
/// together. Retry resumes after the last committed cursor, including on crash.
async fn batch(state: &AppState, id: Uuid, owner: Uuid) -> Result<bool, sqlx::Error> {
    let Some(job) = sqlx::query("SELECT j.*,r.name AS release_name FROM jobs j JOIN releases r ON r.id=j.release_id WHERE j.id=$1 AND j.lease_owner=$2 AND j.state='running' AND j.lease_until>now()")
        .bind(id).bind(owner).fetch_optional(&state.pool).await? else { return Ok(false); };
    let project: Uuid = job.get("project_id");
    let release: String = job.get("release_name");
    let scanned: i32 = job.get("scanned");
    let rows = if job.get::<bool, _>("cancel_requested") {
        Vec::new()
    } else {
        sqlx::query("SELECT id,platform,payload,occurred_at FROM events WHERE project_id=$1 AND release=$2 AND kind='error' AND received_at<=$3 AND ($4::timestamptz IS NULL OR (occurred_at,id)<($4,$5)) ORDER BY occurred_at DESC,id DESC LIMIT $6")
            .bind(project).bind(&release).bind(job.get::<OffsetDateTime,_>("created_at"))
            .bind(job.get::<Option<OffsetDateTime>,_>("cursor_at")).bind(job.get::<Option<Uuid>,_>("cursor_id"))
            .bind(i64::from((20_000-scanned).min(25))).fetch_all(&state.pool).await?
    };
    let mut changes = Vec::new();
    for row in &rows {
        let original: serde_json::Value = row.get("payload");
        let mut payload = original.clone();
        let platform: String = row.get("platform");
        let n = crate::resymbolicate::resolve_payload(
            state,
            project,
            &release,
            &platform,
            &mut payload,
        )
        .await?;
        if n > 0 {
            changes.push((
                row.get::<Uuid, _>("id"),
                original,
                payload,
                i32::try_from(n).unwrap_or(i32::MAX),
            ));
        }
    }
    let mut tx = state.pool.begin().await?;
    let generation: i64 =
        sqlx::query_scalar("SELECT generation FROM instance_settings WHERE singleton FOR SHARE")
            .fetch_one(&mut *tx)
            .await?;
    let active = sqlx::query("SELECT cancel_requested,generation FROM jobs WHERE id=$1 AND lease_owner=$2 AND state='running' AND lease_until>now() FOR UPDATE")
        .bind(id).bind(owner).fetch_optional(&mut *tx).await?;
    let Some(active) = active else {
        return Ok(false);
    };
    if active.get::<bool, _>("cancel_requested") || active.get::<i64, _>("generation") != generation
    {
        sqlx::query("UPDATE jobs SET state=CASE WHEN generation<>$2 THEN 'failed' ELSE 'cancelled' END,error_code=CASE WHEN generation<>$2 THEN 'generation_changed' ELSE NULL END,finished_at=now(),updated_at=now(),lease_owner=NULL,lease_until=NULL WHERE id=$1")
            .bind(id).bind(generation).execute(&mut *tx).await?;
        tx.commit().await?;
        return Ok(false);
    }
    let mut rewritten = 0i32;
    let mut frames = 0i32;
    for (event, original, payload, count) in &changes {
        // Optimistic payload condition also protects a concurrent operator CLI.
        let affected = sqlx::query("UPDATE events SET payload=$1 WHERE id=$2 AND payload=$3")
            .bind(payload)
            .bind(event)
            .bind(original)
            .execute(&mut *tx)
            .await?
            .rows_affected();
        if affected == 1 {
            rewritten += 1;
            frames = frames.saturating_add(*count);
        } else {
            let exists: bool =
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM events WHERE id=$1)")
                    .bind(event)
                    .fetch_one(&mut *tx)
                    .await?;
            if exists {
                return Err(sqlx::Error::Protocol(
                    "concurrent event rewrite; retry batch".to_owned(),
                ));
            }
        }
    }
    let count = i32::try_from(rows.len()).unwrap_or(25);
    let done = rows.len() < 25 || scanned + count >= 20_000;
    let cursor_at = rows
        .last()
        .map(|r| r.get::<OffsetDateTime, _>("occurred_at"))
        .or(job.get("cursor_at"));
    let cursor_id = rows
        .last()
        .map(|r| r.get::<Uuid, _>("id"))
        .or(job.get("cursor_id"));
    sqlx::query("UPDATE jobs SET scanned=scanned+$2,rewritten=rewritten+$3,frames=frames+$4,cursor_at=$5,cursor_id=$6,state=CASE WHEN $7 THEN 'succeeded' ELSE 'running' END,lease_owner=CASE WHEN $7 THEN NULL ELSE lease_owner END,lease_until=CASE WHEN $7 THEN NULL ELSE now()+interval '120 seconds' END,finished_at=CASE WHEN $7 THEN now() ELSE NULL END,updated_at=now() WHERE id=$1")
        .bind(id).bind(count).bind(rewritten).bind(frames).bind(cursor_at).bind(cursor_id).bind(done).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(!done)
}
