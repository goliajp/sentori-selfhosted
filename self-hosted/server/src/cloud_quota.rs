//! Server-side Cloud admission. Self-hosted calls bypass this module entirely.
//! The entitlement row serializes quota checks with writes across all tokens
//! and replicas. Admission leases never extend beyond the paid service period.
use crate::{session_mw::SessionContext, state::AppState};
use axum::{
    Extension, Json,
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Transaction};
use std::{sync::Arc, time::Duration};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Limits {
    monthly_events: i64,
    monthly_replays: i64,
    monthly_ingest_bytes: i64,
    artifact_bytes: i64,
    projects: i64,
    event_retention_days: i64,
    replay_retention_days: i64,
    events_per_second: i64,
}
impl Limits {
    fn valid(&self) -> bool {
        [
            self.monthly_events,
            self.monthly_replays,
            self.monthly_ingest_bytes,
            self.artifact_bytes,
            self.projects,
            self.event_retention_days,
            self.replay_retention_days,
            self.events_per_second,
        ]
        .iter()
        .all(|n| (1..=9_000_000_000_000_000).contains(n))
            && self.projects <= 100_000
            && self.event_retention_days <= 3650
            && self.replay_retention_days <= self.event_retention_days
            && self.events_per_second <= 1_000_000
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("quota database failure: {0}")]
    Database(#[from] sqlx::Error),
    #[error("{0}")]
    Denied(&'static str),
    #[error("{0}")]
    Exhausted(&'static str),
    #[error("Cloud service entitlement unavailable")]
    Unavailable,
}
impl Error {
    pub fn reply(&self) -> (StatusCode, Json<Value>) {
        let (status, code) = match self {
            Self::Database(_) | Self::Unavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "cloud_entitlement_unavailable",
            ),
            Self::Denied(code) => (StatusCode::FORBIDDEN, *code),
            Self::Exhausted(code) => (StatusCode::TOO_MANY_REQUESTS, *code),
        };
        (status, Json(json!({"error":code})))
    }
}
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        self.reply().into_response()
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entitlement {
    pub workspace_id: Uuid,
    pub generation: i64,
    active: bool,
    limits: Option<Limits>,
    #[serde(with = "time::serde::rfc3339")]
    issued_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    valid_until: OffsetDateTime,
    #[serde(rename = "entitledUntil", alias = "paidUntil")]
    #[serde(with = "time::serde::rfc3339::option")]
    paid_until: Option<OffsetDateTime>,
}

async fn refresh(state: &AppState) -> Result<(), Error> {
    let cloud = state.cloud.as_ref().ok_or(Error::Unavailable)?;
    let policy = cloud.entitlement().await.map_err(|_| Error::Unavailable)?;
    let now = OffsetDateTime::now_utc();
    if policy.workspace_id != cloud.workspace
        || policy.generation != cloud.generation
        || policy.issued_at > now + time::Duration::seconds(5)
        || policy.valid_until > policy.issued_at + time::Duration::seconds(60)
    {
        return Err(Error::Unavailable);
    }
    let limits = policy
        .limits
        .filter(Limits::valid)
        .ok_or(Error::Unavailable)?;
    let active = policy.active
        && policy.paid_until.is_some_and(|until| until > now)
        && policy.valid_until > now;
    let until = policy.valid_until.min(policy.paid_until.unwrap_or(now));
    let limits = serde_json::to_value(limits).map_err(|_| Error::Unavailable)?;
    sqlx::query("INSERT INTO cloud_entitlement(singleton,active,limits,valid_until,paid_until,issued_at) VALUES(true,$1,$2,$3,$4,$5) ON CONFLICT(singleton) DO UPDATE SET active=excluded.active,limits=excluded.limits,valid_until=excluded.valid_until,paid_until=excluded.paid_until,issued_at=excluded.issued_at,checked_at=now() WHERE cloud_entitlement.issued_at<=excluded.issued_at")
        .bind(active).bind(limits).bind(until).bind(policy.paid_until).bind(policy.issued_at).execute(&state.pool).await?;
    Ok(())
}

pub fn spawn(state: Arc<AppState>, retention: bool) {
    if state.cloud.is_none() {
        return;
    }
    tokio::spawn(async move {
        let mut last_prune: Option<std::time::Instant> = None;
        loop {
            if let Err(error) = refresh(&state).await {
                tracing::warn!(%error,"Cloud entitlement refresh failed");
            }
            if retention && last_prune.is_none_or(|last| last.elapsed() >= Duration::from_secs(60))
            {
                if let Err(error) = prune(&state.pool).await {
                    tracing::warn!(%error,"Cloud retention pass failed");
                }
                last_prune = Some(std::time::Instant::now());
            }
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    });
}

pub async fn authorize(pool: &PgPool) -> Result<(), Error> {
    let row:Option<(bool,bool)>=sqlx::query_as("SELECT active,coalesce(valid_until>clock_timestamp() AND paid_until>clock_timestamp(),false) FROM cloud_entitlement WHERE singleton")
        .fetch_optional(pool).await?;
    match row {
        Some((true, true)) => Ok(()),
        Some((false, _)) => Err(Error::Denied("workspace_suspended")),
        _ => Err(Error::Unavailable),
    }
}

/// Applied inside each token-authenticated router, after token verification.
pub async fn admission(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    if state.cloud.is_some()
        && let Err(error) = authorize(&state.pool).await
    {
        return error.into_response();
    }
    next.run(request).await
}

async fn lock(tx: &mut Transaction<'_, Postgres>) -> Result<Limits, Error> {
    let row:Option<(bool,bool,Value)>=sqlx::query_as("SELECT active,coalesce(valid_until>clock_timestamp() AND paid_until>clock_timestamp(),false),limits FROM cloud_entitlement WHERE singleton FOR UPDATE")
        .fetch_optional(&mut **tx).await?;
    let Some((active, current, value)) = row else {
        return Err(Error::Unavailable);
    };
    if !active {
        return Err(Error::Denied("workspace_suspended"));
    }
    if !current {
        return Err(Error::Unavailable);
    }
    let limits: Limits = serde_json::from_value(value).map_err(|_| Error::Unavailable)?;
    if !limits.valid() {
        return Err(Error::Unavailable);
    }
    Ok(limits)
}

pub async fn hold(tx: &mut Transaction<'_, Postgres>) -> Result<(), Error> {
    lock(tx).await?;
    Ok(())
}

async fn charge(
    tx: &mut Transaction<'_, Postgres>,
    limits: &Limits,
    events: i64,
    replays: i64,
    bytes: i64,
) -> Result<(), Error> {
    let value:Option<(i64,i64,i64)>=sqlx::query_as("INSERT INTO cloud_monthly_usage(month,events,replays,ingest_bytes) VALUES(date_trunc('month',now() AT TIME ZONE 'UTC')::date,$1,$2,$3) ON CONFLICT(month) DO UPDATE SET events=cloud_monthly_usage.events+$1,replays=cloud_monthly_usage.replays+$2,ingest_bytes=cloud_monthly_usage.ingest_bytes+$3 RETURNING events,replays,ingest_bytes")
        .bind(events).bind(replays).bind(bytes).fetch_optional(&mut **tx).await?;
    let (total_events, total_replays, total_bytes) = value.ok_or(Error::Unavailable)?;
    if events > 0 && total_events > limits.monthly_events {
        return Err(Error::Exhausted("monthly_event_quota"));
    }
    if replays > 0 && total_replays > limits.monthly_replays {
        return Err(Error::Exhausted("monthly_replay_quota"));
    }
    if bytes > 0 && total_bytes > limits.monthly_ingest_bytes {
        return Err(Error::Exhausted("monthly_ingest_quota"));
    }
    if events > 0 {
        let count:i64=sqlx::query_scalar("INSERT INTO cloud_second_usage(second,events) VALUES(date_trunc('second',clock_timestamp()),$1) ON CONFLICT(second) DO UPDATE SET events=cloud_second_usage.events+$1 RETURNING events")
            .bind(events).fetch_one(&mut **tx).await?;
        if count > limits.events_per_second {
            return Err(Error::Exhausted("event_rate_quota"));
        }
    }
    Ok(())
}

/// Count admitted normalized JSON bytes before symbolication. Retransmissions
/// consume traffic quota; an event ID only consumes event quota on first insert.
pub async fn payload(state: &AppState, value: &impl Serialize) -> Result<(), Error> {
    if state.cloud.is_none() {
        return Ok(());
    }
    let bytes = serde_json::to_vec(value)
        .map_err(|_| Error::Unavailable)?
        .len();
    let mut tx = state.pool.begin().await?;
    let limits = lock(&mut tx).await?;
    charge(
        &mut tx,
        &limits,
        0,
        0,
        i64::try_from(bytes).map_err(|_| Error::Unavailable)?,
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn event(tx: &mut Transaction<'_, Postgres>, event: Uuid) -> Result<(), Error> {
    let limits = lock(tx).await?;
    let existing: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM events WHERE id=$1 FOR KEY SHARE")
            .bind(event)
            .fetch_optional(&mut **tx)
            .await?;
    if existing.is_none() {
        charge(tx, &limits, 1, 0, 0).await?;
    }
    Ok(())
}

pub async fn attachment(
    tx: &mut Transaction<'_, Postgres>,
    project: Uuid,
    event: Uuid,
    kind: &str,
    bytes: usize,
) -> Result<(), Error> {
    let limits = lock(tx).await?;
    let replay = if matches!(kind, "replay" | "screens") {
        sqlx::query("INSERT INTO cloud_replay_usage(project_id,event_id) VALUES($1,$2) ON CONFLICT DO NOTHING")
            .bind(project).bind(event).execute(&mut **tx).await?.rows_affected()!=0
    } else {
        false
    };
    charge(
        tx,
        &limits,
        0,
        i64::from(replay),
        i64::try_from(bytes).map_err(|_| Error::Unavailable)?,
    )
    .await
}

pub async fn project(tx: &mut Transaction<'_, Postgres>) -> Result<(), Error> {
    let limits = lock(tx).await?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM projects")
        .fetch_one(&mut **tx)
        .await?;
    if count >= limits.projects {
        return Err(Error::Exhausted("project_quota"));
    }
    Ok(())
}

pub async fn artifact(
    tx: &mut Transaction<'_, Postgres>,
    release: Uuid,
    kind: &str,
    name: &str,
    bytes: usize,
) -> Result<(), Error> {
    let limits = lock(tx).await?;
    let bytes = i64::try_from(bytes).map_err(|_| Error::Unavailable)?;
    let used:i64=sqlx::query_scalar("SELECT coalesce(sum(size_bytes),0)::bigint FROM release_artifacts WHERE NOT(release_id=$1 AND kind=$2 AND name=$3)")
        .bind(release).bind(kind).bind(name).fetch_one(&mut **tx).await?;
    if used
        .checked_add(bytes)
        .is_none_or(|size| size > limits.artifact_bytes)
    {
        return Err(Error::Exhausted("artifact_storage_quota"));
    }
    charge(tx, &limits, 0, 0, bytes).await
}

pub async fn usage(
    State(state): State<Arc<AppState>>,
    Extension(actor): Extension<SessionContext>,
) -> Result<Json<Value>, Error> {
    if !actor.role.is_superadmin() {
        return Err(Error::Denied("owner_required"));
    }
    if state.cloud.is_none() {
        return Err(Error::Denied("not_cloud"));
    }
    let value:Value=sqlx::query_scalar("SELECT jsonb_build_object('limits',e.limits,'active',e.active AND e.valid_until>clock_timestamp(),'checkedAt',e.checked_at,'validUntil',e.valid_until,'paidUntil',e.paid_until,'month',date_trunc('month',now() AT TIME ZONE 'UTC')::date,'resetsAt',(date_trunc('month',now() AT TIME ZONE 'UTC')+interval '1 month') AT TIME ZONE 'UTC','events',coalesce(u.events,0),'replays',coalesce(u.replays,0),'ingestBytes',coalesce(u.ingest_bytes,0),'artifactBytes',(SELECT coalesce(sum(size_bytes),0)::bigint FROM release_artifacts),'projects',(SELECT count(*) FROM projects)) FROM cloud_entitlement e LEFT JOIN cloud_monthly_usage u ON u.month=date_trunc('month',now() AT TIME ZONE 'UTC')::date WHERE e.singleton")
        .fetch_optional(&state.pool).await?.ok_or(Error::Unavailable)?;
    Ok(Json(value))
}

async fn prune(pool: &PgPool) -> Result<(), Error> {
    let mut tx = pool.begin().await?;
    let value: Option<Value> =
        sqlx::query_scalar("SELECT limits FROM cloud_entitlement WHERE singleton FOR UPDATE")
            .fetch_optional(&mut *tx)
            .await?;
    let Some(value) = value else { return Ok(()) };
    let limits: Limits = serde_json::from_value(value).map_err(|_| Error::Unavailable)?;
    if !limits.valid() {
        return Err(Error::Unavailable);
    }
    crate::archive_worker::prune_cloud(
        &mut tx,
        limits.event_retention_days,
        limits.replay_retention_days,
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Acquire admission before a transfer locks or replaces its staged projects.
pub async fn transfer_lock(tx: &mut Transaction<'_, Postgres>) -> Result<(), Error> {
    lock(tx).await?;
    Ok(())
}
/// Historical imports consume project/storage capacity, not current SDK traffic.
/// Check inside the same transaction, before any imported rows become visible.
pub async fn transfer_check(tx: &mut Transaction<'_, Postgres>) -> Result<(), Error> {
    let limits = lock(tx).await?;
    let (projects, bytes): (i64, i64) = sqlx::query_as("SELECT (SELECT count(*) FROM projects),(SELECT coalesce(sum(size_bytes),0)::bigint FROM release_artifacts)")
        .fetch_one(&mut **tx).await?;
    if projects > limits.projects {
        return Err(Error::Exhausted("project_quota"));
    }
    if bytes > limits.artifact_bytes {
        return Err(Error::Exhausted("artifact_storage_quota"));
    }
    Ok(())
}
