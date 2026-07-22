//! `sentorictl` — Sentori operator CLI.
//!
//! Five subcommands matching the v0.1 D6 charter:
//!
//! - `dump` — full workspace snapshot to a directory of
//!   per-table .ndjson files.
//! - `restore` — load a `dump` snapshot back into a fresh
//!   DB (idempotent on PK conflict via INSERT…ON
//!   CONFLICT).
//! - `export` — selective export by table or project.
//! - `import` — selective load of an `export` payload.
//! - `migrate` — apply pending migrations from
//!   `core/migrations/` (mirrors what `sentori-server`
//!   runs at boot, useful for ops-driven schema upgrades
//!   without the server running).
//!
//! Connection: `--db postgres://…` flag or `DATABASE_URL`
//! env var.

#![forbid(unsafe_code)]
#![allow(
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions
)]

use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand};
use sqlx::PgPool;

mod commands;

#[derive(Parser, Debug)]
#[command(name = "sentorictl", version, about, long_about = None)]
struct Cli {
    /// Postgres connection URL.
    /// Falls back to env `DATABASE_URL` /
    /// `SENTORI_DATABASE_URL`.
    #[arg(long, env = "SENTORI_DATABASE_URL", global = true)]
    db: Option<String>,

    /// Suppress non-error output.
    #[arg(long, global = true)]
    quiet: bool,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Snapshot the entire workspace to a directory.
    Dump {
        /// Target directory (created if missing).
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Load a `dump` snapshot back into the DB.
    Restore {
        /// Source directory produced by `dump`.
        #[arg(short, long)]
        from: PathBuf,
        /// Overwrite existing rows on PK conflict.
        #[arg(long, default_value_t = false)]
        overwrite: bool,
    },
    /// Export selected tables / projects to NDJSON.
    Export {
        /// Output file (- for stdout).
        #[arg(short, long, default_value = "-")]
        out: String,
        /// Comma-separated tables to include (default: all).
        #[arg(long)]
        tables: Option<String>,
        /// Restrict to one project id (UUID).
        #[arg(long)]
        project: Option<uuid::Uuid>,
    },
    /// Import an NDJSON file produced by `export`.
    Import {
        /// Input file (- for stdin).
        #[arg(short, long, default_value = "-")]
        input: String,
    },
    /// Show current schema version + pending migrations.
    Status,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let db_url = cli
        .db
        .clone()
        .or_else(|| std::env::var("DATABASE_URL").ok())
        .context("Postgres URL required (--db or DATABASE_URL env)")?;
    let pool = PgPool::connect(&db_url).await.context("db connect")?;

    match cli.cmd {
        Cmd::Dump { out } => commands::dump::run(&pool, &out, cli.quiet).await?,
        Cmd::Restore { from, overwrite } => {
            commands::restore::run(&pool, &from, overwrite, cli.quiet).await?;
        }
        Cmd::Export { out, tables, project } => {
            commands::export::run(&pool, &out, tables.as_deref(), project, cli.quiet).await?;
        }
        Cmd::Import { input } => commands::import::run(&pool, &input, cli.quiet).await?,
        Cmd::Status => commands::status::run(&pool).await?,
    }
    Ok(())
}
