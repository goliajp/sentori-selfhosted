//! Sentori operator CLI: encrypted full recovery archives, independent
//! verification, offline restore/resume, and selective NDJSON data transfer.
//! `export`/`import` are not complete recovery backups.
//! Connection: `--db`, `SENTORI_DATABASE_URL`, or `DATABASE_URL`.

#![forbid(unsafe_code)]
#![allow(
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions
)]

use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand};
mod commands;
mod db;
mod secret_env;

#[derive(Parser, Debug)]
#[command(name = "sentori-operator", version, about, long_about = None)]
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
    /// Reset a stopped instance after creating a fresh verified independent backup.
    Reset(commands::reset::Args),
    /// Finish interrupted blob cleanup using the durable external reset journal.
    FinishReset {
        #[arg(long)]
        journal: PathBuf,
        #[arg(long)]
        key_file: PathBuf,
    },
    /// Create a verified encrypted recovery archive, including blob bytes.
    Dump {
        /// New archive file; existing files are never overwritten.
        #[arg(short, long)]
        out: PathBuf,
        #[arg(long)]
        blob_dir: PathBuf,
        #[arg(long)]
        key_file: PathBuf,
    },
    /// Create an encrypted project package without instance credentials.
    PackageProjects {
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        blob_dir: PathBuf,
        #[arg(long)]
        key_file: PathBuf,
        /// Explicit project UUIDs; repeat the option for multiple projects.
        #[arg(long, required = true)]
        project: Vec<uuid::Uuid>,
    },
    /// Restore into an empty migrated database while its server is stopped.
    Restore {
        /// Encrypted archive file produced by `dump`.
        #[arg(short, long)]
        from: PathBuf,
        /// New blob directory; configure the target server to use it afterwards.
        #[arg(long)]
        blob_dir: PathBuf,
        #[arg(long)]
        key_file: PathBuf,
        /// Rejected: recovery never overwrites an existing instance.
        #[arg(long, default_value_t = false)]
        overwrite: bool,
    },
    /// Verify all archive authentication, schema entries, hashes and blob bytes.
    Verify {
        #[arg(long)]
        from: PathBuf,
        #[arg(long)]
        key_file: PathBuf,
    },
    /// Generate a private 256-bit recovery key file. Keep it separately.
    Keygen {
        #[arg(long)]
        out: PathBuf,
    },
    /// Apply the migrations shipped with this binary to a database.
    Migrate,
    /// Resume automatic workers after recovery validation; both servers must be stopped.
    ResumeRecovery {
        #[arg(long)]
        instance_id: uuid::Uuid,
        #[arg(long)]
        generation: i64,
        #[arg(long)]
        archive_id: uuid::Uuid,
        #[arg(long)]
        confirm_old_instance_stopped: bool,
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
    /// Read-only instance diagnostics; exit 0 pass, 1 warning, 2 fail, 3 unknown.
    Healthcheck {
        /// Persistent blob directory; defaults to `SENTORI_ATTACHMENT_STORE=fs:...`.
        #[arg(long)]
        blob_dir: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    if let Cmd::Healthcheck { blob_dir } = &cli.cmd {
        return run_healthcheck(&cli, blob_dir.as_deref()).await;
    }
    match &cli.cmd {
        Cmd::Keygen { out } => return commands::dump::keygen(out),
        Cmd::Verify { from, key_file } => {
            return commands::restore::verify(from, key_file, cli.quiet);
        }
        _ => {}
    }
    let db_url = required_database_url(&cli)?;
    let pool = db::connect(&db_url).await.context("db connect")?;
    // All ordinary operators cooperate with the same offline boundary as servers.
    let _instance_lock = if matches!(
        cli.cmd,
        Cmd::Reset(_) | Cmd::FinishReset { .. } | Cmd::Restore { .. } | Cmd::ResumeRecovery { .. }
    ) {
        None
    } else {
        let guard = sentori_snapshot::reset::shared_guard(&pool).await?;
        sentori_snapshot::reset::ensure_available(&pool).await?;
        Some(guard)
    };

    match cli.cmd {
        Cmd::Reset(args) => commands::reset::run(&pool, &args, cli.quiet).await?,
        Cmd::FinishReset { journal, key_file } => {
            let report = sentori_snapshot::reset::finish(
                &pool,
                &journal,
                commands::dump::read_key(&key_file)?,
            )
            .await?;
            if !cli.quiet {
                println!("{}", serde_json::to_string_pretty(&report)?);
            }
        }
        Cmd::Dump {
            out,
            blob_dir,
            key_file,
        } => commands::dump::run(&pool, &out, &blob_dir, &key_file, cli.quiet).await?,
        Cmd::PackageProjects {
            out,
            blob_dir,
            key_file,
            project,
        } => {
            let manifest = sentori_snapshot::capture::capture_projects(
                &pool,
                &blob_dir,
                &out,
                commands::dump::read_key(&key_file)?,
                include_str!("../../../VERSION").trim(),
                &project,
                &sentori_snapshot::capture::CaptureControl::default(),
            )
            .await?;
            if !cli.quiet {
                println!("{}", serde_json::to_string_pretty(&manifest)?);
            }
        }
        Cmd::Restore {
            from,
            blob_dir,
            key_file,
            overwrite,
        } => {
            commands::restore::run(&pool, &from, &blob_dir, &key_file, overwrite, cli.quiet)
                .await?;
        }
        Cmd::Migrate => sqlx::migrate!("../../core/migrations").run(&pool).await?,
        Cmd::ResumeRecovery {
            instance_id,
            generation,
            archive_id,
            confirm_old_instance_stopped,
        } => {
            anyhow::ensure!(
                confirm_old_instance_stopped,
                "--confirm-old-instance-stopped is required after verifying the recovered instance"
            );
            sentori_snapshot::restore::resume(&pool, instance_id, generation, archive_id).await?;
        }
        Cmd::Keygen { .. } | Cmd::Verify { .. } | Cmd::Healthcheck { .. } => {}
        Cmd::Export {
            out,
            tables,
            project,
        } => {
            commands::export::run(&pool, &out, tables.as_deref(), project, cli.quiet).await?;
        }
        Cmd::Import { input } => commands::import::run(&pool, &input, cli.quiet).await?,
        Cmd::Status => commands::status::run(&pool).await?,
    }
    Ok(())
}

async fn run_healthcheck(cli: &Cli, blob_dir: Option<&std::path::Path>) -> anyhow::Result<()> {
    let db_url = (|| -> anyhow::Result<Option<String>> {
        if let Some(url) = cli.db.clone().filter(|v| !v.trim().is_empty()) {
            return Ok(Some(url));
        }
        if let Some(url) = secret_env::read("SENTORI_DATABASE_URL")? {
            return Ok(Some(url));
        }
        secret_env::read("DATABASE_URL")
    })()
    .ok()
    .flatten();
    let pool = if let Some(url) = db_url {
        tokio::time::timeout(std::time::Duration::from_secs(3), db::connect(&url))
            .await
            .ok()
            .and_then(Result::ok)
    } else {
        None
    };
    let path = blob_dir.map(std::path::Path::to_path_buf).or_else(|| {
        std::env::var("SENTORI_ATTACHMENT_STORE")
            .ok()
            .and_then(|v| v.strip_prefix("fs:").map(PathBuf::from))
    });
    let report = sentori_snapshot::healthcheck::inspect(pool.as_ref(), path.as_deref()).await;
    println!("{}", serde_json::to_string_pretty(&report)?);
    std::process::exit(report.exit_code());
}

fn required_database_url(cli: &Cli) -> anyhow::Result<String> {
    let mut db_url = cli.db.clone().filter(|value| !value.trim().is_empty());
    if db_url.is_none() {
        db_url = secret_env::read("SENTORI_DATABASE_URL")?;
    }
    if db_url.is_none() {
        db_url = secret_env::read("DATABASE_URL")?;
    }
    db_url.context("Postgres URL required (--db or DATABASE_URL environment/secret file)")
}
