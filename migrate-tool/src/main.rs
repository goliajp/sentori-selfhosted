//! sentori-migrate — business-layer ETL from legacy DB to v0.2 DB.
//!
//! Reads legacy sentori production tables, transforms identity-
//! layer fields (orgs.id → workspaces.id, memberships.role 4→3 via
//! viewer→user mapping), then 1:1 INSERTs project-scoped business
//! rows. Idempotent on PK ON CONFLICT DO NOTHING so re-runs are
//! safe.
//!
//! Usage:
//!   sentori-migrate \
//!     --src "postgres://sentori:pass@legacy-host/sentori" \
//!     --dst "postgres://sentori:pass@v02-host/sentori" \
//!     [--dry-run]
//!     [--workspace-id <uuid>]   # force all orgs into one workspace
//!     [--tables identity,events,issues,...]

use anyhow::{Context, Result};
use clap::Parser;
use sqlx::{PgPool, postgres::PgPoolOptions};
use tracing::{error, info};

mod identity;
mod report;
mod tables;

#[derive(Parser, Debug)]
#[command(name = "sentori-migrate", version, about = "Legacy → v0.2 ETL")]
struct Cli {
    /// Source (legacy) DB URL.
    #[arg(long)]
    src: String,
    /// Destination (v0.2) DB URL.
    #[arg(long)]
    dst: String,
    /// Dry-run: count rows that would migrate without writing.
    #[arg(long)]
    dry_run: bool,
    /// Comma-separated table sets to migrate. Default: all.
    /// Available sets: identity, tokens, releases, events, issues,
    /// sessions, spans, push, attachments, all.
    #[arg(long, default_value = "all")]
    tables: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    let cli = Cli::parse();

    info!(
        src = %redact(&cli.src),
        dst = %redact(&cli.dst),
        dry_run = cli.dry_run,
        tables = %cli.tables,
        "sentori-migrate starting",
    );

    let src = connect(&cli.src).await.context("connect src")?;
    let dst = connect(&cli.dst).await.context("connect dst")?;

    let sets: Vec<&str> = if cli.tables == "all" {
        vec![
            "identity",
            "tokens",
            "releases",
            // issues BEFORE events — events.issue_id FK references
            // issues(id); the reverse order leaves events at 0 rows.
            "issues",
            "events",
            "sessions",
            "spans",
            "push",
            "attachments",
            "dashboard",
            "dashboard_extra",
            "analytics",
            "metrics",
            "ops",
            "billing",
            "identity_extras",
            "notifications_email",
            "workflow",
            "misc",
            "saas",
        ]
    } else {
        cli.tables.split(',').map(str::trim).collect()
    };

    let mut report = report::Report::default();

    for set in sets {
        info!(set, "migrating set");
        let result: Result<u64> = match set {
            "identity" => identity::migrate_all(&src, &dst, cli.dry_run, &mut report).await,
            "tokens" => tables::tokens::migrate(&src, &dst, cli.dry_run, &mut report).await,
            "releases" => tables::releases::migrate(&src, &dst, cli.dry_run, &mut report).await,
            "events" => tables::events::migrate(&src, &dst, cli.dry_run, &mut report).await,
            "issues" => tables::issues::migrate(&src, &dst, cli.dry_run, &mut report).await,
            "sessions" => tables::sessions::migrate(&src, &dst, cli.dry_run, &mut report).await,
            "spans" => tables::spans::migrate(&src, &dst, cli.dry_run, &mut report).await,
            "push" => tables::push::migrate(&src, &dst, cli.dry_run, &mut report).await,
            "attachments" => {
                tables::attachments::migrate(&src, &dst, cli.dry_run, &mut report).await
            }
            "dashboard" => {
                tables::dashboard::migrate(&src, &dst, cli.dry_run, &mut report).await
            }
            "dashboard_extra" => {
                tables::dashboard_extra::migrate(&src, &dst, cli.dry_run, &mut report).await
            }
            "analytics" => {
                tables::analytics::migrate(&src, &dst, cli.dry_run, &mut report).await
            }
            "metrics" => {
                tables::metrics::migrate(&src, &dst, cli.dry_run, &mut report).await
            }
            "ops" => tables::ops::migrate(&src, &dst, cli.dry_run, &mut report).await,
            "billing" => tables::billing::migrate(&src, &dst, cli.dry_run, &mut report).await,
            "identity_extras" => {
                tables::identity_extras::migrate(&src, &dst, cli.dry_run, &mut report).await
            }
            "notifications_email" => {
                tables::notifications_email::migrate(&src, &dst, cli.dry_run, &mut report).await
            }
            "workflow" => tables::workflow::migrate(&src, &dst, cli.dry_run, &mut report).await,
            "misc" => tables::misc::migrate(&src, &dst, cli.dry_run, &mut report).await,
            "saas" => tables::saas::migrate(&src, &dst, cli.dry_run, &mut report).await,
            other => {
                error!(set = %other, "unknown table set");
                continue;
            }
        };
        match result {
            Ok(n) => info!(set, rows = n, "set complete"),
            Err(e) => error!(set, error = %e, "set failed"),
        }
    }

    report.print();
    info!("sentori-migrate done");
    Ok(())
}

fn init_tracing() {
    use tracing_subscriber::EnvFilter;
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn")),
        )
        .init();
}

async fn connect(url: &str) -> Result<PgPool> {
    PgPoolOptions::new()
        .max_connections(8)
        .connect(url)
        .await
        .map_err(Into::into)
}

/// Hide password component of a postgres URL for logging.
fn redact(url: &str) -> String {
    if let Some(at) = url.find('@') {
        if let Some(scheme_end) = url.find("://") {
            let creds_start = scheme_end + 3;
            return format!("{}***{}", &url[..creds_start], &url[at..]);
        }
    }
    url.to_string()
}
