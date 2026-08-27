//! `sentorictl status` — schema version + row counts.

use anyhow::Context;
use sqlx::PgPool;

use super::TABLES;
use sqlx::AssertSqlSafe;

pub async fn run(pool: &PgPool) -> anyhow::Result<()> {
    // Schema version from sqlx's bookkeeping table.
    let latest: Option<(i64,)> =
        sqlx::query_as("SELECT version FROM _sqlx_migrations ORDER BY version DESC LIMIT 1")
            .fetch_optional(pool)
            .await
            .context("read _sqlx_migrations")?;
    println!(
        "schema version: {}",
        latest
            .map(|(v,)| v.to_string())
            .unwrap_or_else(|| "none".into())
    );
    println!();
    println!("row counts:");
    for table in TABLES {
        let sql = format!("SELECT COUNT(*)::bigint FROM {table}");
        // Audited: the only interpolation is a table name from
        // `commands::TABLES`, a compile-time list. Nothing a caller
        // types reaches the statement.
        match sqlx::query_as::<_, (i64,)>(AssertSqlSafe(sql))
            .fetch_one(pool)
            .await
        {
            Ok((n,)) => println!("  {table:<32} {n:>10}"),
            Err(e) => println!("  {table:<32} ERR {e}"),
        }
    }
    Ok(())
}
