//! `sentorictl dump --out <dir>` — full snapshot.
//!
//! Writes one `<table>.ndjson` per Sentori table into
//! `<dir>`, plus a `manifest.json` with the table list +
//! per-table row counts + timestamp.

use std::path::Path;

use anyhow::Context;
use serde_json::Value;
use sqlx::{PgPool, Row};
use time::OffsetDateTime;
use tokio::fs;
use tokio::io::AsyncWriteExt;

use super::TABLES;

pub async fn run(pool: &PgPool, out: &Path, quiet: bool) -> anyhow::Result<()> {
    fs::create_dir_all(out).await.context("create dump dir")?;
    let mut manifest_entries: Vec<serde_json::Value> = Vec::new();

    for table in TABLES {
        let count = dump_one(pool, table, out, quiet).await?;
        manifest_entries.push(serde_json::json!({
            "table": table,
            "rows": count,
        }));
    }

    let manifest = serde_json::json!({
        "version": "0.1.0",
        "created_at": OffsetDateTime::now_utc().to_string(),
        "tables": manifest_entries,
    });
    let manifest_path = out.join("manifest.json");
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)
        .await
        .context("write manifest")?;
    if !quiet {
        println!("✅ dump complete: {} tables → {}", TABLES.len(), out.display());
    }
    Ok(())
}

async fn dump_one(
    pool: &PgPool,
    table: &str,
    out: &Path,
    quiet: bool,
) -> anyhow::Result<i64> {
    // Use row_to_json so caller doesn't need per-table
    // typed mapping. The full row JSON includes every
    // column verbatim.
    let sql = format!("SELECT row_to_json(t) FROM {table} t");
    let rows = sqlx::query(&sql)
        .fetch_all(pool)
        .await
        .with_context(|| format!("scan {table}"))?;

    let path = out.join(format!("{table}.ndjson"));
    let mut f = fs::File::create(&path).await.context("create file")?;
    let mut count: i64 = 0;
    for row in &rows {
        let v: Value = row.get(0);
        let line = format!("{v}\n");
        f.write_all(line.as_bytes()).await.context("write row")?;
        count += 1;
    }
    if !quiet {
        println!("  {table:<32} {count:>8}");
    }
    Ok(count)
}
