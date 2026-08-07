//! Daily [`PartitionLifecycle`] for `runtime_metrics_raw`.
//!
//! Copy of K6's `sentori-span-store::PartitionLifecycle`
//! adapted to day grain. Per K9 design lock 2026-06-20, we
//! deliberately keep the duplicate until retro-K4 events
//! partition lands (3-4 consumers = trigger to extract a
//! shared `partition-lifecycle` stone).
//!
//! Differences vs K6:
//! - **Day grain**: child tables named `runtime_metrics_raw_YYYY_MM_DD`
//!   (vs K6's `spans_YYYY_MM`).
//! - **Forward window default 3 days** (vs K6's 6 months) —
//!   metrics are written in real time; we just need late SDK
//!   batches' destination partition.
//! - **Retention default 90 days** (vs K6's 14 days) —
//!   metrics history is the long-tail value path.

use std::collections::HashSet;

use sqlx::{PgPool, Row};
use time::{Date, OffsetDateTime, format_description::FormatItem, macros::format_description};

use crate::error::RuntimeMetricsError;

/// The parent table we manage. Plain const so untrusted input
/// never flows into the format-string SQL below.
pub(crate) const RAW_PARENT: &str = "runtime_metrics_raw";

/// Partition-name day suffix format (`YYYY_MM_DD`).
const DAY_FMT: &[FormatItem<'_>] = format_description!("[year]_[month]_[day]");
/// SQL boundary literal for partition `FROM`/`TO`.
const DAY_BOUND_FMT: &[FormatItem<'_>] = format_description!("[year]-[month]-[day] 00:00:00+00");

/// Sub-handle on [`crate::MetricsStore`].
#[derive(Debug, Clone)]
pub struct PartitionLifecycle<'a> {
    pool: &'a PgPool,
}

impl<'a> PartitionLifecycle<'a> {
    pub(crate) const fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// Create daily partitions for `now`'s day + `days_ahead`
    /// days. Returns the count of newly-created partitions.
    ///
    /// Idempotent — re-running on the same `now` creates
    /// zero. Recommended cadence: caller spawns a 1h tick
    /// that calls `ensure_future(now_utc, 3)`.
    ///
    /// # Errors
    ///
    /// - [`RuntimeMetricsError::Db`] on backend failure.
    pub async fn ensure_future(
        &self,
        now: OffsetDateTime,
        days_ahead: u32,
    ) -> Result<u32, RuntimeMetricsError> {
        let existing = child_partitions(self.pool, RAW_PARENT).await?;
        let mut created = 0u32;
        let today = now.date();
        for offset in 0..=i64::from(days_ahead) {
            let day = today + time::Duration::days(offset);
            let next = day + time::Duration::days(1);
            let suffix = day.format(DAY_FMT).map_err(|e| {
                RuntimeMetricsError::InvalidInput(format!("partition day format: {e}"))
            })?;
            let name = format!("{RAW_PARENT}_{suffix}");
            if existing.contains(&name) {
                continue;
            }
            let from = day.format(DAY_BOUND_FMT).map_err(|e| {
                RuntimeMetricsError::InvalidInput(format!("from-bound format: {e}"))
            })?;
            let to = next
                .format(DAY_BOUND_FMT)
                .map_err(|e| RuntimeMetricsError::InvalidInput(format!("to-bound format: {e}")))?;
            let sql = format!(
                "CREATE TABLE IF NOT EXISTS {name} PARTITION OF {RAW_PARENT} \
                 FOR VALUES FROM ('{from}') TO ('{to}')"
            );
            sqlx::query(&sql).execute(self.pool).await?;
            created += 1;
        }
        Ok(created)
    }

    /// Drop daily partitions whose upper bound is ≤ `cutoff`.
    /// Returns the count of dropped partitions.
    ///
    /// `runtime_metrics_raw_default` is never dropped.
    ///
    /// # Errors
    ///
    /// - [`RuntimeMetricsError::InvalidPartitionName`] for a
    ///   relation in `pg_inherits` that doesn't parse to
    ///   `_YYYY_MM_DD`.
    /// - [`RuntimeMetricsError::Db`] on backend failure.
    pub async fn drop_before(&self, cutoff: OffsetDateTime) -> Result<u32, RuntimeMetricsError> {
        let existing = child_partitions(self.pool, RAW_PARENT).await?;
        let mut dropped = 0u32;
        for name in existing {
            if name == "runtime_metrics_raw_default" {
                continue;
            }
            let day = parse_partition_day(&name, RAW_PARENT)?;
            let upper = day + time::Duration::days(1);
            let upper_dt = OffsetDateTime::new_utc(upper, time::Time::MIDNIGHT);
            if upper_dt <= cutoff {
                let sql = format!("DROP TABLE IF EXISTS {name}");
                sqlx::query(&sql).execute(self.pool).await?;
                dropped += 1;
            }
        }
        Ok(dropped)
    }

    /// Enumerate current child partition names.
    ///
    /// # Errors
    ///
    /// [`RuntimeMetricsError::Db`] on database failure.
    pub async fn list_existing(&self) -> Result<Vec<String>, RuntimeMetricsError> {
        let mut v: Vec<String> = child_partitions(self.pool, RAW_PARENT)
            .await?
            .into_iter()
            .collect();
        v.sort();
        Ok(v)
    }
}

async fn child_partitions(pool: &PgPool, parent: &str) -> Result<HashSet<String>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT c.relname AS name FROM pg_inherits i \
         JOIN pg_class p ON i.inhparent = p.oid \
         JOIN pg_class c ON i.inhrelid = c.oid \
         WHERE p.relname = $1",
    )
    .bind(parent)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| r.get::<String, _>("name"))
        .collect())
}

/// Parse a `<parent>_YYYY_MM_DD` partition name back to a [`Date`].
fn parse_partition_day(name: &str, parent: &str) -> Result<Date, RuntimeMetricsError> {
    let prefix = format!("{parent}_");
    let suffix = name
        .strip_prefix(&prefix)
        .ok_or_else(|| RuntimeMetricsError::InvalidPartitionName(name.to_string()))?;
    Date::parse(suffix, DAY_FMT)
        .map_err(|_| RuntimeMetricsError::InvalidPartitionName(name.to_string()))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn parse_partition_day_ok() {
        let d = parse_partition_day("runtime_metrics_raw_2026_01_05", RAW_PARENT).unwrap();
        assert_eq!(d.year(), 2026);
        assert_eq!(u8::from(d.month()), 1);
        assert_eq!(d.day(), 5);
    }

    #[test]
    fn parse_partition_day_rejects_default() {
        assert!(parse_partition_day("runtime_metrics_raw_default", RAW_PARENT).is_err());
    }

    #[test]
    fn parse_partition_day_rejects_garbage() {
        assert!(parse_partition_day("runtime_metrics_raw_2026_13_01", RAW_PARENT).is_err());
        assert!(parse_partition_day("garbage", RAW_PARENT).is_err());
    }
}
