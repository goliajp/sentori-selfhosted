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
        None => TABLES.to_vec(),
    };
    let mut writer: Box<dyn AsyncWrite + Send + Unpin> = if out == "-" {
        Box::new(tokio::io::stdout())
    } else {
        Box::new(tokio::fs::File::create(out).await.context("open out")?)
    };

    let mut total: u64 = 0;
    for table in &selected {
        let n = export_one(pool, table, writer.as_mut(), project).await?;
        if !quiet {
            eprintln!("  {table:<32} exported {n}");
        }
        total += n;
    }
    writer.flush().await?;
    if !quiet {
        eprintln!("✅ exported {total} rows across {} tables", selected.len());
    }
    Ok(())
}

async fn export_one(
    pool: &PgPool,
    table: &str,
    writer: &mut (dyn AsyncWrite + Send + Unpin),
    project: Option<uuid::Uuid>,
) -> anyhow::Result<u64> {
    let sql = match project {
        Some(_) if has_project_id(table) => {
            format!("SELECT row_to_json(t) FROM {table} t WHERE project_id = $1")
        }
        _ => format!("SELECT row_to_json(t) FROM {table} t"),
    };

    let rows = if project.is_some() && has_project_id(table) {
        // Audited: the only interpolation is a table name from
        // `commands::TABLES`, a compile-time list. Nothing a caller
        // types reaches the statement.
        sqlx::query(AssertSqlSafe(sql.clone()))
            .bind(project)
            .fetch_all(pool)
            .await
            .with_context(|| format!("scan {table}"))?
    } else {
        // Audited: the only interpolation is a table name from
        // `commands::TABLES`, a compile-time list. Nothing a caller
        // types reaches the statement.
        sqlx::query(AssertSqlSafe(sql.clone()))
            .fetch_all(pool)
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
    "audit_logs",
    "project_assignments",
    "tokens",
    "issues",
    "events",
    "event_attachments",
    "releases",
    "probes",
    "assert_stats",
    "notification_prefs",
    "delivery_log",
    "push_tokens",
    "push_credentials",
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
