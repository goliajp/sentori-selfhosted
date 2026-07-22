//! [`MetricsStore`] — the public handle.

use sentori_workspace_identity::ProjectId;
use sqlx::{PgPool, Row};
use time::{Duration, OffsetDateTime};

use crate::error::RuntimeMetricsError;
use crate::model::{DropReason, DroppedRow, MetricPoint, RollupRow, RollupTier, row_to_rollup};
use crate::partitions::PartitionLifecycle;

/// Public handle.
#[derive(Debug, Clone)]
pub struct MetricsStore {
    pool: PgPool,
}

impl MetricsStore {
    /// Construct.
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Borrow the pool.
    #[must_use]
    pub const fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Partition + retention sub-handle. See
    /// [`PartitionLifecycle`].
    #[must_use]
    pub const fn partitions(&self) -> PartitionLifecycle<'_> {
        PartitionLifecycle::new(&self.pool)
    }

    // ── ingest ──────────────────────────────────────────────

    /// Write a batch of [`MetricPoint`]s into
    /// `runtime_metrics_raw`. Returns the count of distinct
    /// rows that landed (duplicates dedup via PK).
    ///
    /// Per-row INSERT inside one transaction. For high-volume
    /// ingest the K9 follow-up will swap in a `COPY FROM`
    /// path; v0.1 keeps the simple INSERT for typed-binding
    /// + easy FK error translation.
    ///
    /// # Errors
    ///
    /// - [`RuntimeMetricsError::InvalidInput`] for empty batch
    ///   or non-finite value.
    /// - [`RuntimeMetricsError::ProjectNotFound`] on FK
    ///   violation.
    /// - [`RuntimeMetricsError::Db`] on other backend error.
    pub async fn ingest_batch(&self, points: &[MetricPoint]) -> Result<usize, RuntimeMetricsError> {
        if points.is_empty() {
            return Ok(0);
        }
        for p in points {
            validate_point(p)?;
        }

        let mut tx = self.pool.begin().await?;
        let mut written = 0usize;
        // Project ids whose INSERT wrote nothing. Either the row deduped on
        // the PK, or the project doesn't exist and the driving SELECT matched
        // zero rows — the two are indistinguishable at this point and neither
        // raises an FK violation. Resolved once after the loop.
        let mut wrote_nothing: Vec<ProjectId> = Vec::new();
        for p in points {
            let tags_hash = p.tags_hash();
            let tags_value = serde_json::Value::Object(p.tags.clone());
            let result = sqlx::query(
                r"
                INSERT INTO runtime_metrics_raw
                    (ts, workspace_id, project_id, name, value, tags, tags_hash,
                     release, environment, device_class)
                SELECT $1, p.workspace_id, $2, $3, $4, $5, $6, $7, $8, $9
                FROM projects p WHERE p.id = $2
                ON CONFLICT (project_id, ts, name, tags_hash) DO NOTHING
                ",
            )
            .bind(p.ts)
            .bind(p.project_id.into_uuid())
            .bind(&p.name)
            .bind(p.value)
            .bind(&tags_value)
            .bind(tags_hash)
            .bind(p.release.as_deref())
            .bind(p.environment.as_deref())
            .bind(p.device_class.as_deref())
            .execute(&mut *tx)
            .await
            .map_err(|e| translate_fk(e, p.project_id))?;
            if result.rows_affected() == 0 {
                if !wrote_nothing.contains(&p.project_id) {
                    wrote_nothing.push(p.project_id);
                }
            } else {
                written += result.rows_affected() as usize;
            }
        }

        for pid in wrote_nothing {
            let exists: Option<(uuid::Uuid,)> =
                sqlx::query_as("SELECT id FROM projects WHERE id = $1")
                    .bind(pid.into_uuid())
                    .fetch_optional(&mut *tx)
                    .await?;
            if exists.is_none() {
                return Err(RuntimeMetricsError::ProjectNotFound(pid.into_uuid()));
            }
        }

        tx.commit().await?;
        Ok(written)
    }

    // ── rollups ─────────────────────────────────────────────

    /// Roll [`window_start`, `window_end`) of `runtime_metrics_raw`
    /// into `runtime_metrics_1m`.
    ///
    /// Returns the count of (project, name, bucket, dims) rows
    /// UPSERTed. Idempotent — re-running the same window
    /// produces the same row set.
    ///
    /// Caller picks the window (typically `[now - 70s,
    /// now - 10s)` to skip in-flight late SDK batches).
    ///
    /// # Errors
    ///
    /// [`RuntimeMetricsError::Db`] on backend failure.
    pub async fn roll_raw_to_1m(
        &self,
        window_start: OffsetDateTime,
        window_end: OffsetDateTime,
    ) -> Result<u64, RuntimeMetricsError> {
        let result = sqlx::query(
            r"
            INSERT INTO runtime_metrics_1m (
                bucket_ts, workspace_id, project_id, name,
                release, environment, device_class,
                count, sum, avg, p50, p95, p99
            )
            SELECT
                date_trunc('minute', ts)             AS bucket_ts,
                workspace_id,
                project_id,
                name,
                COALESCE(release, '')                AS release,
                COALESCE(environment, '')            AS environment,
                COALESCE(device_class, '')           AS device_class,
                count(*)                             AS count,
                sum(value)                           AS sum,
                avg(value)                           AS avg,
                percentile_cont(0.5)  WITHIN GROUP (ORDER BY value) AS p50,
                percentile_cont(0.95) WITHIN GROUP (ORDER BY value) AS p95,
                percentile_cont(0.99) WITHIN GROUP (ORDER BY value) AS p99
            FROM runtime_metrics_raw
            WHERE ts >= $1 AND ts < $2
            GROUP BY 1, 2, 3, 4, 5, 6, 7
            ON CONFLICT (project_id, bucket_ts, name, release, environment, device_class)
            DO UPDATE SET
                count = EXCLUDED.count,
                sum   = EXCLUDED.sum,
                avg   = EXCLUDED.avg,
                p50   = EXCLUDED.p50,
                p95   = EXCLUDED.p95,
                p99   = EXCLUDED.p99
            ",
        )
        .bind(window_start)
        .bind(window_end)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Roll [`bucket_start`, `bucket_end`) of
    /// `runtime_metrics_1m` into `runtime_metrics_1h`.
    ///
    /// Same shape as [`Self::roll_raw_to_1m`]. Caller passes
    /// a 1-hour window (typically the previous full hour).
    ///
    /// # Errors
    ///
    /// [`RuntimeMetricsError::Db`] on backend failure.
    pub async fn roll_1m_to_1h(
        &self,
        bucket_start: OffsetDateTime,
        bucket_end: OffsetDateTime,
    ) -> Result<u64, RuntimeMetricsError> {
        let result = sqlx::query(
            r"
            INSERT INTO runtime_metrics_1h (
                bucket_ts, workspace_id, project_id, name,
                release, environment, device_class,
                count, sum, avg, p50, p95, p99
            )
            SELECT
                date_trunc('hour', bucket_ts)                  AS bucket_ts,
                workspace_id, project_id, name, release, environment, device_class,
                sum(count)                                     AS count,
                sum(sum)                                       AS sum,
                sum(sum) / NULLIF(sum(count), 0)               AS avg,
                -- 1h percentile = count-weighted avg of 1m
                -- p50/p95/p99 (approximate; error bounded
                -- ~5 percent for typical SDK distributions;
                -- the dashboard renders _1h as the 1h
                -- aggregate, not the exact p99).
                sum(p50 * count) / NULLIF(sum(count), 0)       AS p50,
                sum(p95 * count) / NULLIF(sum(count), 0)       AS p95,
                sum(p99 * count) / NULLIF(sum(count), 0)       AS p99
            FROM runtime_metrics_1m
            WHERE bucket_ts >= $1 AND bucket_ts < $2
            GROUP BY 1, 2, 3, 4, 5, 6, 7
            ON CONFLICT (project_id, bucket_ts, name, release, environment, device_class)
            DO UPDATE SET
                count = EXCLUDED.count,
                sum   = EXCLUDED.sum,
                avg   = EXCLUDED.avg,
                p50   = EXCLUDED.p50,
                p95   = EXCLUDED.p95,
                p99   = EXCLUDED.p99
            ",
        )
        .bind(bucket_start)
        .bind(bucket_end)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Roll [`bucket_start`, `bucket_end`) of
    /// `runtime_metrics_1h` into `runtime_metrics_1d`.
    ///
    /// # Errors
    ///
    /// [`RuntimeMetricsError::Db`] on backend failure.
    pub async fn roll_1h_to_1d(
        &self,
        bucket_start: OffsetDateTime,
        bucket_end: OffsetDateTime,
    ) -> Result<u64, RuntimeMetricsError> {
        let result = sqlx::query(
            r"
            INSERT INTO runtime_metrics_1d (
                bucket_ts, workspace_id, project_id, name,
                release, environment, device_class,
                count, sum, avg, p50, p95, p99
            )
            SELECT
                date_trunc('day', bucket_ts)                   AS bucket_ts,
                workspace_id, project_id, name, release, environment, device_class,
                sum(count)                                     AS count,
                sum(sum)                                       AS sum,
                sum(sum) / NULLIF(sum(count), 0)               AS avg,
                sum(p50 * count) / NULLIF(sum(count), 0)       AS p50,
                sum(p95 * count) / NULLIF(sum(count), 0)       AS p95,
                sum(p99 * count) / NULLIF(sum(count), 0)       AS p99
            FROM runtime_metrics_1h
            WHERE bucket_ts >= $1 AND bucket_ts < $2
            GROUP BY 1, 2, 3, 4, 5, 6, 7
            ON CONFLICT (project_id, bucket_ts, name, release, environment, device_class)
            DO UPDATE SET
                count = EXCLUDED.count,
                sum   = EXCLUDED.sum,
                avg   = EXCLUDED.avg,
                p50   = EXCLUDED.p50,
                p95   = EXCLUDED.p95,
                p99   = EXCLUDED.p99
            ",
        )
        .bind(bucket_start)
        .bind(bucket_end)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    // ── read ────────────────────────────────────────────────

    /// Query a single rollup tier for one (project, name) over
    /// `[from, to)`. Caller picks the tier.
    ///
    /// Returns rows sorted by `bucket_ts` ascending. Empty if
    /// nothing matches.
    ///
    /// # Errors
    ///
    /// [`RuntimeMetricsError::Db`] on backend failure.
    pub async fn query(
        &self,
        project_id: ProjectId,
        name: &str,
        tier: RollupTier,
        from: OffsetDateTime,
        to: OffsetDateTime,
    ) -> Result<Vec<RollupRow>, RuntimeMetricsError> {
        let sql = format!(
            "SELECT bucket_ts, project_id, name, release, environment, device_class,
                    count, sum, avg, p50, p95, p99
             FROM {tier_table}
             WHERE project_id = $1 AND name = $2
               AND bucket_ts >= $3 AND bucket_ts < $4
             ORDER BY bucket_ts ASC",
            tier_table = tier.table_name(),
        );
        let rows = sqlx::query(&sql)
            .bind(project_id.into_uuid())
            .bind(name)
            .bind(from)
            .bind(to)
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(row_to_rollup).collect()
    }

    // ── dropped counters ────────────────────────────────────

    /// Bump (or insert) the per-day drop counter for
    /// `(project, today, reason)`.
    ///
    /// Caller resolves `day` themselves so test rigs can drive
    /// it deterministically; production code passes
    /// `OffsetDateTime::now_utc().date()`.
    ///
    /// # Errors
    ///
    /// - [`RuntimeMetricsError::ProjectNotFound`] if the
    ///   project doesn't exist.
    /// - [`RuntimeMetricsError::Db`] on backend failure.
    pub async fn record_drop(
        &self,
        project_id: ProjectId,
        day: time::Date,
        reason: DropReason,
        delta: i64,
    ) -> Result<(), RuntimeMetricsError> {
        let written: Option<(uuid::Uuid,)> = sqlx::query_as(
            r"
            INSERT INTO runtime_metrics_dropped (day, workspace_id, project_id, reason, count)
            SELECT $1, p.workspace_id, $2, $3, $4
            FROM projects p WHERE p.id = $2
            ON CONFLICT (project_id, day, reason)
            DO UPDATE SET count = runtime_metrics_dropped.count + EXCLUDED.count
            RETURNING project_id
            ",
        )
        .bind(day)
        .bind(project_id.into_uuid())
        .bind(reason.as_db_str())
        .bind(delta)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| translate_fk(e, project_id))?;
        // Unknown project → the driving SELECT matches zero rows → nothing is
        // inserted, the ON CONFLICT branch never runs and no FK violation is
        // raised. The DO UPDATE branch still RETURNINGs, so a missing row here
        // means only one thing: the project doesn't exist.
        if written.is_none() {
            return Err(RuntimeMetricsError::ProjectNotFound(project_id.into_uuid()));
        }
        Ok(())
    }

    /// Read the drop counters for a project over a date range
    /// `[from_day, to_day]` (inclusive).
    ///
    /// # Errors
    ///
    /// [`RuntimeMetricsError::Db`] / `InvalidDropReasonInDb`.
    pub async fn list_drops(
        &self,
        project_id: ProjectId,
        from_day: time::Date,
        to_day: time::Date,
    ) -> Result<Vec<DroppedRow>, RuntimeMetricsError> {
        let rows = sqlx::query(
            r"
            SELECT day, project_id, reason, count
            FROM runtime_metrics_dropped
            WHERE project_id = $1
              AND day >= $2 AND day <= $3
            ORDER BY day ASC, reason ASC
            ",
        )
        .bind(project_id.into_uuid())
        .bind(from_day)
        .bind(to_day)
        .fetch_all(&self.pool)
        .await?;
        rows.iter()
            .map(|r| {
                let reason_str: &str = r.get("reason");
                Ok(DroppedRow {
                    day: r.get("day"),
                    project_id: ProjectId::from_uuid(r.get("project_id")),
                    reason: DropReason::from_db_str(reason_str)?,
                    count: r.get("count"),
                })
            })
            .collect()
    }
}

/// Default cascade-tick windows for the caller's
/// `tokio::spawn` loop. Plain consts; consumer crate
/// composes them with its scheduler.
pub mod cadence {
    use super::Duration;

    /// 60 s tick for raw → 1m.
    pub const RAW_TO_1M_TICK: Duration = Duration::seconds(60);
    /// Safety margin so the raw → 1m window skips in-flight
    /// SDK batches.
    pub const RAW_LATE_MARGIN: Duration = Duration::seconds(10);
    /// Window size for raw → 1m (always 70 s; 60 s + 10 s
    /// safety so we don't double-count).
    pub const RAW_WINDOW: Duration = Duration::seconds(70);
}

fn validate_point(p: &MetricPoint) -> Result<(), RuntimeMetricsError> {
    if p.name.trim().is_empty() {
        return Err(RuntimeMetricsError::InvalidInput(
            "metric name must not be empty".into(),
        ));
    }
    if p.name.len() > 200 {
        return Err(RuntimeMetricsError::InvalidInput(format!(
            "metric name too long: {}",
            p.name.len()
        )));
    }
    if !p.value.is_finite() {
        return Err(RuntimeMetricsError::InvalidInput(
            "metric value must be finite (NaN / Inf rejected)".into(),
        ));
    }
    Ok(())
}

fn translate_fk(err: sqlx::Error, project_id: ProjectId) -> RuntimeMetricsError {
    if let sqlx::Error::Database(db_err) = &err
        && db_err.code().as_deref() == Some("23503")
    {
        return RuntimeMetricsError::ProjectNotFound(project_id.into_uuid());
    }
    RuntimeMetricsError::Db(err)
}
