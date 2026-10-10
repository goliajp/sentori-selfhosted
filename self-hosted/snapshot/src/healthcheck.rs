//! Read-only instance diagnostics shared by the HTTP service and operator CLI.
//! No probes, migrations, archive reads, external requests or customer rows are written.

use serde::Serialize;
use sqlx::PgPool;
use std::{path::Path, time::Duration};
use time::OffsetDateTime;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Pass,
    Unknown,
    Warning,
    Fail,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub id: &'static str,
    pub status: Status,
    pub code: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub schema_version: u8,
    pub scope: &'static str,
    pub checked_at: String,
    pub status: Status,
    pub checks: Vec<Check>,
    pub backup_max_age_hours: u32,
}

impl Report {
    pub fn exit_code(&self) -> i32 {
        match self.status {
            Status::Pass => 0,
            Status::Warning => 1,
            Status::Fail => 2,
            Status::Unknown => 3,
        }
    }

    fn add(&mut self, id: &'static str, status: Status, code: &'static str) {
        self.checks.push(Check { id, status, code });
        self.status = self.status.max(status);
    }
}

type AppliedMigration = (i64, Vec<u8>, bool);

fn schema_matches(applied: &[AppliedMigration]) -> bool {
    let expected = crate::expected_migrations();
    applied.len() == expected.len()
        && applied
            .iter()
            .zip(expected)
            .all(|((version, checksum, success), wanted)| {
                *success && *version == wanted.version && hex::encode(checksum) == wanted.checksum
            })
}

/// Inspect one instance. `None` means the caller could not establish a database
/// connection. The report intentionally omits connection strings, paths and SQL errors.
pub async fn inspect(pool: Option<&PgPool>, blob_dir: Option<&Path>) -> Report {
    let mut report = Report {
        schema_version: 1,
        scope: "read-only",
        checked_at: OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default(),
        status: Status::Pass,
        checks: Vec::new(),
        backup_max_age_hours: 24,
    };
    if let Some(pool) = pool {
        let connected = tokio::time::timeout(Duration::from_secs(3), async {
            sqlx::query_scalar::<_, i32>("SELECT 1")
                .fetch_one(pool)
                .await
        })
        .await;
        if matches!(connected, Ok(Ok(1))) {
            report.add("database", Status::Pass, "database_reachable");
            // A failed/timed-out query is unknown, never an empty healthy result.
            let metadata = tokio::time::timeout(Duration::from_secs(10), read_metadata(pool)).await;
            match metadata {
                Ok(Ok(data)) => add_metadata(&mut report, data),
                _ => missing_metadata(&mut report),
            }
        } else {
            report.add("database", Status::Fail, "database_unavailable");
            missing_metadata(&mut report);
        }
    } else {
        report.add("database", Status::Fail, "database_unavailable");
        missing_metadata(&mut report);
    }
    match blob_dir {
        None => report.add("storage", Status::Warning, "storage_not_persistent"),
        Some(path) => {
            let readable = tokio::time::timeout(Duration::from_secs(3), async {
                tokio::fs::read_dir(path).await
            })
            .await;
            if matches!(readable, Ok(Ok(_))) {
                report.add("storage", Status::Pass, "storage_directory_readable");
            } else {
                report.add("storage", Status::Fail, "storage_directory_unreadable");
            }
        }
    }
    // The DB has no independently attested off-host backup or restore drill
    // receipt. Do not turn a successful archive job into a recovery guarantee.
    report.add("recovery", Status::Unknown, "recovery_not_verified");
    report
}

struct Metadata {
    migrations: Vec<AppliedMigration>,
    instances: Vec<(String, bool)>,
    backup_age_seconds: Option<f64>,
    stalled_operation: bool,
}

async fn read_metadata(pool: &PgPool) -> Result<Metadata, sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout = '3s'")
        .execute(&mut *tx)
        .await?;
    let migrations =
        sqlx::query_as("SELECT version, checksum, success FROM _sqlx_migrations ORDER BY version")
            .fetch_all(&mut *tx)
            .await?;
    let instances =
        sqlx::query_as("SELECT operating_mode, initialized_at IS NOT NULL FROM instance_settings")
            .fetch_all(&mut *tx)
            .await?;
    let backup_age_seconds = sqlx::query_scalar(
        "SELECT EXTRACT(EPOCH FROM now() - max(o.finished_at))::float8 \
         FROM instance_operations o JOIN instance_settings i \
         ON o.instance_id = i.id AND o.generation = i.generation \
         WHERE o.kind = 'backup' AND o.state = 'ready'",
    )
    .fetch_one(&mut *tx)
    .await?;
    let stalled_operation = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM instance_operations WHERE \
         (state = 'running' AND (lease_until IS NULL OR lease_until < now())) OR \
         (state = 'queued' AND created_at < now() - interval '10 minutes'))",
    )
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Metadata {
        migrations,
        instances,
        backup_age_seconds,
        stalled_operation,
    })
}

fn missing_metadata(report: &mut Report) {
    for id in ["schema", "instance", "backup", "operations"] {
        report.add(id, Status::Unknown, "metadata_unavailable");
    }
}

fn add_metadata(report: &mut Report, data: Metadata) {
    if schema_matches(&data.migrations) {
        report.add("schema", Status::Pass, "schema_matches_binary");
    } else {
        report.add("schema", Status::Fail, "schema_mismatch");
    }
    match data.instances.as_slice() {
        [(mode, initialized)] if mode == "normal" && *initialized => {
            report.add("instance", Status::Pass, "instance_ready");
        }
        [(mode, _)] if mode == "restore_validation" => {
            report.add("instance", Status::Warning, "recovery_validation_active");
        }
        [(_, false)] => report.add("instance", Status::Warning, "setup_incomplete"),
        _ => report.add("instance", Status::Fail, "instance_identity_invalid"),
    }
    match data.backup_age_seconds {
        Some(age) if age.is_finite() && (0.0..=86400.0).contains(&age) => {
            report.add("backup", Status::Pass, "recent_backup_record");
        }
        Some(age) if age.is_finite() && age >= 0.0 => {
            report.add("backup", Status::Warning, "backup_record_stale");
        }
        Some(_) => report.add("backup", Status::Unknown, "backup_clock_invalid"),
        None => report.add("backup", Status::Warning, "backup_record_missing"),
    }
    if data.stalled_operation {
        report.add("operations", Status::Warning, "lifecycle_operation_stalled");
    } else {
        report.add(
            "operations",
            Status::Pass,
            "no_stalled_lifecycle_operations",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_failed_extra_and_changed_migrations_are_not_healthy() {
        let good: Vec<_> = crate::expected_migrations()
            .into_iter()
            .map(|m| {
                (
                    m.version,
                    hex::decode(m.checksum).expect("embedded hex"),
                    true,
                )
            })
            .collect();
        assert!(schema_matches(&good));
        let mut changed = good.clone();
        changed[0].2 = false;
        assert!(!schema_matches(&changed));
        changed[0].2 = true;
        changed[0].1.push(0);
        assert!(!schema_matches(&changed));
        assert!(!schema_matches(&good[1..]));
        let mut extra = good;
        extra.push((99999, vec![], true));
        assert!(!schema_matches(&extra));
    }

    #[tokio::test]
    async fn unavailable_database_and_storage_fail_without_leaking_paths_or_creating_files() {
        let dir = tempfile::tempdir().expect("temp directory");
        let missing = dir.path().join("private-path-password");
        let report = inspect(None, Some(&missing)).await;
        assert_eq!(report.status, Status::Fail);
        assert_eq!(report.exit_code(), 2);
        assert!(!missing.exists());
        let text = serde_json::to_string(&report).expect("report");
        assert!(!text.contains("private-path-password"));
        assert!(
            report
                .checks
                .iter()
                .any(|c| c.id == "schema" && c.status == Status::Unknown)
        );
        assert!(
            report
                .checks
                .iter()
                .any(|c| c.id == "recovery" && c.status == Status::Unknown)
        );
    }
}
