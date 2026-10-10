//! `sentorictl export` — selective NDJSON export by table
//! / project. Output is a single .ndjson stream where each
//! line is `{"table": "<name>", "row": {...}}`.

use anyhow::Context;
use serde_json::Value;
use sqlx::{PgPool, Row};
use tokio::io::{AsyncWrite, AsyncWriteExt};

use super::TABLES;
use sqlx::AssertSqlSafe;

pub async fn run(
    pool: &PgPool,
    out: &str,
    tables: Option<&str>,
    project: Option<uuid::Uuid>,
    quiet: bool,
) -> anyhow::Result<()> {
    let selected: Vec<&str> = match tables {
        Some(s) => s.split(',').map(str::trim).collect(),
        None => TABLES
            .iter()
            .copied()
            .filter(|table| project.is_none() || project_filter(table).is_some())
            .collect(),
    };
    for table in &selected {
        anyhow::ensure!(TABLES.contains(table), "unknown export table: {table}");
        if project.is_some() {
            anyhow::ensure!(
                project_filter(table).is_some(),
                "{table} is instance-scoped; it cannot be included in a project export"
            );
        }
    }
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let mut writer: Box<dyn AsyncWrite + Send + Unpin> = if out == "-" {
        Box::new(tokio::io::stdout())
    } else {
        Box::new(tokio::fs::File::create(out).await.context("open out")?)
    };

    let mut total: u64 = 0;
    for table in &selected {
        let n = export_one(&mut tx, table, writer.as_mut(), project).await?;
        if !quiet {
            eprintln!("  {table:<32} exported {n}");
        }
        total += n;
    }
    writer.flush().await?;
    tx.commit().await?;
    if !quiet {
        eprintln!("✅ exported {total} rows across {} tables", selected.len());
    }
    Ok(())
}

async fn export_one(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    table: &str,
    writer: &mut (dyn AsyncWrite + Send + Unpin),
    project: Option<uuid::Uuid>,
) -> anyhow::Result<u64> {
    let sql = match (project, project_filter(table)) {
        (Some(_), Some(filter)) => format!("SELECT row_to_json(t) FROM {table} t WHERE {filter}"),
        (Some(_), None) => anyhow::bail!("instance-scoped table in project export"),
        (None, _) => format!("SELECT row_to_json(t) FROM {table} t"),
    };

    let rows = if project.is_some() {
        // Audited: the only interpolation is a table name from
        // `commands::TABLES`, a compile-time list. Nothing a caller
        // types reaches the statement.
        sqlx::query(AssertSqlSafe(sql.clone()))
            .bind(project)
            .fetch_all(&mut **tx)
            .await
            .with_context(|| format!("scan {table}"))?
    } else {
        // Audited: the only interpolation is a table name from
        // `commands::TABLES`, a compile-time list. Nothing a caller
        // types reaches the statement.
        sqlx::query(AssertSqlSafe(sql.clone()))
            .fetch_all(&mut **tx)
            .await
            .with_context(|| format!("scan {table}"))?
    };

    let mut count: u64 = 0;
    for row in &rows {
        let v: Value = row.get(0);
        let envelope = serde_json::json!({"table": table, "row": v});
        let line = format!("{envelope}\n");
        writer.write_all(line.as_bytes()).await?;
        count += 1;
    }
    Ok(count)
}

/// Tables that carry a `project_id` column we can filter on.
///
/// Read from `core/migrations` rather than remembered: the previous
/// list named eighteen tables and sixteen of them no longer existed,
/// so `export --project` filtered two and silently exported nothing
/// for the rest.
const PROJECT_SCOPED: &[&str] = &[
    "cloud_replay_usage",
    "project_transfer_routes",
    "audit_logs",
    "project_assignments",
    "tokens",
    "issues",
    "events",
    "sessions",
    "event_attachments",
    "releases",
    "jobs",
    "idempotency_requests",
    "tasks",
    "task_commands",
    "monitors",
    "monitor_commands",
    "outbound_integrations",
    "integration_commands",
    "notification_events",
    "notification_deliveries",
    "task_integrations",
    "task_signal_deliveries",
    "inbox_items",
    "probes",
    "assert_stats",
    "notification_prefs",
    "delivery_log",
    "push_tokens",
    "push_credentials",
    "push_commands",
    "push_audience_previews",
    "device_tokens",
    "push_sends",
    "push_preferences",
    "backend_checks",
];

/// `PROJECT_SCOPED`, for the schema-agreement test in `commands::mod`.
///
/// The list stays private — nothing outside this module should filter
/// by it — but a test one module up has to be able to compare it
/// against `core/migrations`, because the list going stale is exactly
/// what happened and nothing else can see it.
#[cfg(test)]
pub(super) fn project_scoped_for_test() -> &'static [&'static str] {
    PROJECT_SCOPED
}

fn has_project_id(table: &str) -> bool {
    PROJECT_SCOPED.contains(&table)
}

fn project_filter(table: &str) -> Option<&'static str> {
    if has_project_id(table) {
        return Some("t.project_id = $1");
    }
    match table {
        "projects" => Some("t.id = $1"),
        "issue_user_hits" | "issue_activity" => {
            Some("t.issue_id IN (SELECT id FROM issues WHERE project_id = $1)")
        }
        "release_artifacts" => {
            Some("t.release_id IN (SELECT id FROM releases WHERE project_id = $1)")
        }
        "task_events" => Some("t.task_id IN (SELECT id FROM tasks WHERE project_id = $1)"),
        "monitor_samples" | "monitor_events" => {
            Some("t.monitor_id IN (SELECT id FROM monitors WHERE project_id = $1)")
        }
        "push_delivery_logs" => {
            Some("t.send_id IN (SELECT id FROM push_sends WHERE project_id = $1)")
        }
        "device_topics" => {
            Some("t.device_token_id IN (SELECT id FROM device_tokens WHERE project_id = $1)")
        }
        "outbox_events" => Some(
            "t.job_id IN (SELECT id FROM jobs WHERE project_id = $1) OR t.task_event_id IN (SELECT id FROM task_events WHERE task_id IN (SELECT id FROM tasks WHERE project_id = $1))",
        ),
        _ => None,
    }
}
