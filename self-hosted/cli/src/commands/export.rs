//! `sentorictl export` — selective NDJSON export by table
//! / project. Output is a single .ndjson stream where each
//! line is `{"table": "<name>", "row": {...}}`.

use anyhow::Context;
use serde_json::Value;
use sqlx::{PgPool, Row};
use tokio::io::{AsyncWrite, AsyncWriteExt};

use super::TABLES;

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
        Some(_) if has_project_id(table) => format!(
            "SELECT row_to_json(t) FROM {table} t WHERE project_id = $1"
        ),
        _ => format!("SELECT row_to_json(t) FROM {table} t"),
    };

    let rows = if project.is_some() && has_project_id(table) {
        sqlx::query(&sql)
            .bind(project)
            .fetch_all(pool)
            .await
            .with_context(|| format!("scan {table}"))?
    } else {
        sqlx::query(&sql)
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
const PROJECT_SCOPED: &[&str] = &[
    "issues",
    "events",
    "identity_fingerprints",
    "spans",
    "replay_sessions",
    "runtime_metrics_raw",
    "runtime_metrics_1m",
    "runtime_metrics_1h",
    "runtime_metrics_1d",
    "runtime_metrics_dropped",
    "cert_watch_domains",
    "cert_observations",
    "integrations",
    "alert_rules",
    "saved_views",
    "usage_counters",
    "project_user_visibility",
    "project_dropped",
];

fn has_project_id(table: &str) -> bool {
    PROJECT_SCOPED.contains(&table)
}
