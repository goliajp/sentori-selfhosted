//! `sentorictl import` — load an NDJSON stream from
//! `export` back into the DB.
//!
//! Stream format: `{"table": "<name>", "row": {...}}`
//! per line. Each row is inserted with `ON CONFLICT DO
//! NOTHING` semantics (idempotent re-import).

use anyhow::Context;
use serde::Deserialize;
use sqlx::AssertSqlSafe;
use sqlx::PgPool;
use tokio::io::{AsyncBufReadExt, BufReader, stdin};

#[derive(Deserialize)]
struct Envelope {
    table: String,
    row: serde_json::Value,
}

pub async fn run(pool: &PgPool, input: &str, quiet: bool) -> anyhow::Result<()> {
    let reader: Box<dyn tokio::io::AsyncRead + Send + Unpin> = if input == "-" {
        Box::new(stdin())
    } else {
        Box::new(tokio::fs::File::open(input).await.context("open input")?)
    };
    let mut lines = BufReader::new(reader).lines();
    let mut count: u64 = 0;
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let env: Envelope = serde_json::from_str(&line).context("parse envelope")?;
        let sql = format!(
            "INSERT INTO {} SELECT * FROM \
             json_populate_record(NULL::{}, $1::json) \
             ON CONFLICT DO NOTHING",
            env.table, env.table,
        );
        // Audited: the only interpolation is a table name from
        // `commands::TABLES`, a compile-time list. Nothing a caller
        // types reaches the statement.
        sqlx::query(AssertSqlSafe(sql.clone()))
            .bind(env.row.to_string())
            .execute(pool)
            .await
            .with_context(|| format!("import row into {}", env.table))?;
        count += 1;
    }
    if !quiet {
        println!("✅ imported {count} rows");
    }
    Ok(())
}
