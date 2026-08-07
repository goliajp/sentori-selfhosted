//! Partition lifecycle for the RANGE-partitioned `spans` table.
//!
//! First-mover for v0.1's partition pattern (K9 / K4-followup
//! reuse). The shape:
//!
//! - **`ensure_future(months_ahead)`** — `CREATE TABLE IF NOT
//!   EXISTS <table>_YYYY_MM PARTITION OF spans …` for each
//!   month from `now` to `now + months_ahead`. Idempotent;
//!   safe to run from a daily cron + at startup.
//! - **`drop_before(cutoff)`** — `DROP TABLE` per monthly
//!   child whose upper bound ≤ cutoff. O(1) per partition, vs
//!   O(rows) for `DELETE WHERE received_at < cutoff` which
//!   would hammer autovacuum.
//! - **`prune_traces_before(cutoff)`** + **`prune_orphan_traces`**
//!   work on the `traces` table (which can't be partitioned
//!   because of the `trace_id` unique constraint).
//!
//! Partition naming: `<parent>_YYYY_MM`. The lifecycle reads
//! `pg_inherits` to find what already exists so re-runs don't
//! flap or error on duplicates.

use std::collections::HashSet;

use sqlx::{PgPool, Row};
use time::{Duration, Month, OffsetDateTime};

use crate::error::SpanStoreError;

/// The parent table we manage. Plain string constant rather
/// than runtime-supplied so we control the values that ever
/// flow into the format-string CREATE / DROP.
pub(crate) const SPANS_PARENT: &str = "spans";

/// Sub-handle on [`crate::SpanStore`].
#[derive(Debug, Clone)]
pub struct PartitionLifecycle<'a> {
    pool: &'a PgPool,
}

impl<'a> PartitionLifecycle<'a> {
    pub(crate) const fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// Create monthly partitions for the next `months_ahead`
    /// months starting at `now`'s month. Returns the count of
    /// newly-created partitions (existing ones are skipped).
    ///
    /// Call from server startup + from a daily janitor cron.
    ///
    /// # Errors
    ///
    /// [`SpanStoreError::Db`] on database failure.
    pub async fn ensure_future(
        &self,
        now: OffsetDateTime,
        months_ahead: u32,
    ) -> Result<u32, SpanStoreError> {
        let existing = child_partitions(self.pool, SPANS_PARENT).await?;
        let mut created = 0u32;
        for offset in 0..months_ahead {
            let (y, m) = add_months(now.year(), u8::from(now.month()), offset);
            let (ny, nm) = next_month(y, m);
            let name = format!("{SPANS_PARENT}_{y:04}_{m:02}");
            if existing.contains(&name) {
                continue;
            }
            // `SPANS_PARENT` is a hardcoded constant; y/m/ny/nm
            // are integers we generated. Injection-safe by
            // construction.
            let sql = format!(
                "CREATE TABLE IF NOT EXISTS {name} PARTITION OF {SPANS_PARENT} \
                 FOR VALUES FROM ('{y:04}-{m:02}-01') TO ('{ny:04}-{nm:02}-01')"
            );
            sqlx::query(&sql).execute(self.pool).await?;
            created += 1;
        }
        Ok(created)
    }

    /// Drop monthly partitions whose upper bound is ≤ `cutoff`.
    /// Returns the count of dropped partitions.
    ///
    /// O(1) per partition (TRUNCATE-equivalent + catalog
    /// update). The `spans_default` catch-all is never
    /// dropped — leaving it in place keeps writes safe
    /// across calendar boundaries before `ensure_future` has
    /// run.
    ///
    /// # Errors
    ///
    /// - [`SpanStoreError::InvalidPartitionName`] if a child
    ///   relation in `pg_inherits` doesn't match the
    ///   `spans_YYYY_MM` shape (shouldn't happen unless an
    ///   operator hand-rolled a partition).
    /// - [`SpanStoreError::Db`] on database failure.
    pub async fn drop_before(&self, cutoff: OffsetDateTime) -> Result<u32, SpanStoreError> {
        let existing = child_partitions(self.pool, SPANS_PARENT).await?;
        let mut dropped = 0u32;
        for name in existing {
            if name == "spans_default" {
                continue;
            }
            let (year, month) = parse_partition_name(&name, SPANS_PARENT)?;
            // Upper bound is the first day of the NEXT month.
            let (ny, nm) = next_month(year, month);
            let upper = month_first_day(ny, nm);
            if upper <= cutoff {
                let sql = format!("DROP TABLE IF EXISTS {name}");
                sqlx::query(&sql).execute(self.pool).await?;
                dropped += 1;
            }
        }
        Ok(dropped)
    }

    /// `DELETE FROM traces WHERE last_seen < cutoff`. Returns
    /// the deleted row count. Used as the retention sweep for
    /// the `traces` rollup table (which can't be partitioned
    /// — see migration 0005's traces section).
    ///
    /// # Errors
    ///
    /// [`SpanStoreError::Db`] on database failure.
    pub async fn prune_traces_before(&self, cutoff: OffsetDateTime) -> Result<u64, SpanStoreError> {
        let result = sqlx::query("DELETE FROM traces WHERE last_seen < $1")
            .bind(cutoff)
            .execute(self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Delete "orphan" traces — rows where `root_op IS NULL`
    /// AND `last_seen < now - grace`. The grace window lets a
    /// late-arriving root span (slow network, retry) still
    /// patch the row before we consider it abandoned.
    ///
    /// # Errors
    ///
    /// [`SpanStoreError::Db`] on database failure.
    pub async fn prune_orphan_traces(
        &self,
        now: OffsetDateTime,
        grace_hours: i64,
    ) -> Result<u64, SpanStoreError> {
        let cutoff = now - Duration::hours(grace_hours);
        let result = sqlx::query("DELETE FROM traces WHERE root_op IS NULL AND last_seen < $1")
            .bind(cutoff)
            .execute(self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Enumerate current child partition names.
    ///
    /// # Errors
    ///
    /// [`SpanStoreError::Db`] on database failure.
    pub async fn list_existing(&self) -> Result<Vec<String>, SpanStoreError> {
        let mut v: Vec<String> = child_partitions(self.pool, SPANS_PARENT)
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

/// Parse a `<parent>_YYYY_MM` partition name back to `(year, month)`.
fn parse_partition_name(name: &str, parent: &str) -> Result<(i32, u8), SpanStoreError> {
    let prefix = format!("{parent}_");
    let suffix = name
        .strip_prefix(&prefix)
        .ok_or_else(|| SpanStoreError::InvalidPartitionName(name.to_string()))?;
    // Expect "YYYY_MM" exactly: 4 digit + '_' + 2 digit.
    if suffix.len() != 7 || suffix.as_bytes()[4] != b'_' {
        return Err(SpanStoreError::InvalidPartitionName(name.to_string()));
    }
    let year: i32 = suffix[..4]
        .parse()
        .map_err(|_| SpanStoreError::InvalidPartitionName(name.to_string()))?;
    let month: u8 = suffix[5..]
        .parse()
        .map_err(|_| SpanStoreError::InvalidPartitionName(name.to_string()))?;
    if !(1..=12).contains(&month) {
        return Err(SpanStoreError::InvalidPartitionName(name.to_string()));
    }
    Ok((year, month))
}

/// Add `delta` months to `(year, month)`, wrapping the month
/// at 12 and carrying into year.
fn add_months(year: i32, month: u8, delta: u32) -> (i32, u8) {
    let total = i64::from(month) - 1 + i64::from(delta);
    let added_years = total / 12;
    let new_month = u8::try_from(total % 12 + 1).unwrap_or(1);
    (year + added_years as i32, new_month)
}

/// `(y, m) → (y, m+1)` with year-wrap.
const fn next_month(year: i32, month: u8) -> (i32, u8) {
    if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    }
}

/// `(y, m)` → first-day midnight UTC `OffsetDateTime`.
fn month_first_day(year: i32, month: u8) -> OffsetDateTime {
    use time::Date;
    let m = Month::try_from(month).unwrap_or(Month::January);
    // year out of `time` crate's range → far-future fallback
    // so we never drop a partition by accident on a malformed
    // name (those go through the typed error path instead).
    let date = Date::from_calendar_date(year, m, 1).unwrap_or(Date::MAX);
    OffsetDateTime::new_utc(date, time::Time::MIDNIGHT)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn parse_partition_name_ok() {
        let (y, m) = parse_partition_name("spans_2026_07", "spans").unwrap();
        assert_eq!(y, 2026);
        assert_eq!(m, 7);
    }

    #[test]
    fn parse_partition_name_rejects_default() {
        assert!(parse_partition_name("spans_default", "spans").is_err());
    }

    #[test]
    fn parse_partition_name_rejects_garbage() {
        assert!(parse_partition_name("spans_2026_13", "spans").is_err());
        assert!(parse_partition_name("spans_2026", "spans").is_err());
        assert!(parse_partition_name("foo", "spans").is_err());
    }

    #[test]
    fn add_months_basic() {
        assert_eq!(add_months(2026, 1, 0), (2026, 1));
        assert_eq!(add_months(2026, 1, 6), (2026, 7));
        assert_eq!(add_months(2026, 6, 7), (2027, 1));
        assert_eq!(add_months(2026, 12, 1), (2027, 1));
    }

    #[test]
    fn next_month_wraps() {
        assert_eq!(next_month(2026, 12), (2027, 1));
        assert_eq!(next_month(2026, 1), (2026, 2));
    }
}
