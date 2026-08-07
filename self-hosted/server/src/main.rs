//! Sentori self-hosted server binary (v1).
//!
//! Single-tenant by design: one instance, one customer. Boot order:
//! migrate (compile-time-embedded core/migrations) → reconcile the
//! env-declared owner → build AppState → spawn push + archive
//! workers → serve the five-kind ingest, the AI api surface, and
//! the dashboard from one router.
//!
//! Image goal: < 80 MB distroless cc + strip.
//! Startup goal: < 30s under `docker compose up`.

#![forbid(unsafe_code)]
#![allow(
    clippy::doc_markdown,
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::missing_const_for_fn,
    clippy::module_name_repetitions
)]

use std::sync::Arc;

use anyhow::Context;
use axum::Router;
use sqlx::PgPool;
use tokio::net::TcpListener;
use tracing::info;

mod apns;
mod archive_worker;
mod audit;
mod backend_check_worker;
mod backfill_split;
mod blob_store;
mod bootstrap;
mod bundle;
mod client_ip;
mod env_config;
mod fcm;
mod handlers;
mod hcm;
mod mailer;
mod mipush;
mod native_symbolicate;
mod notify;
mod pipeline;
mod push_quarantine;
mod push_worker;
mod rate_limit;
mod security_headers;
mod session_mw;
mod state;
mod symbolicate;
mod token_cache;
mod webpush;
mod webpush_encrypt;
mod wire_time;

use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let db_url = env_config::env_or_file("SENTORI_DATABASE_URL")
        .or_else(|| env_config::env_or_file("DATABASE_URL"))
        .context("SENTORI_DATABASE_URL (or DATABASE_URL) env var required")?;

    // `sentori-server reset-password <email>` — the operator path
    // for a locked-out owner (design.md §10): no SMTP dependency,
    // prints a fresh password to stdout and exits.
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("reset-password") {
        let email = args
            .get(2)
            .context("usage: sentori-server reset-password <email>")?;
        let pool = PgPool::connect(&db_url).await.context("db connect")?;
        return bootstrap::reset_password(&pool, email).await;
    }

    // `sentori-server backfill-split` — one-shot: split pre-2.9.0
    // mixed issues by environment × platform (see backfill_split.rs).
    if args.get(1).map(String::as_str) == Some("backfill-split") {
        return backfill_split::run(&db_url)
            .await
            .map_err(|e| anyhow::anyhow!("backfill-split: {e}"));
    }

    let bind = std::env::var("SENTORI_BIND").unwrap_or_else(|_| "0.0.0.0:8080".to_string());

    info!(%bind, "sentori self-hosted server boot");

    let pool = PgPool::connect(&db_url).await.context("db connect")?;
    run_migrations(&pool).await.context("migrate")?;

    // Env-declared owner reconcile (idempotent, declarative).
    if let Err(e) = bootstrap::ensure_owner(&pool).await {
        tracing::warn!(error = %e, "owner bootstrap skipped");
    }

    let attachments = blob_store::AttachmentStore::from_env()
        .await
        .context("attachment store init")?;
    let state = Arc::new(AppState::new(pool.clone(), attachments));

    // Background workers: push dispatch + retention archive.
    let token_cache = std::sync::Arc::new(token_cache::TokenCache::new());
    push_worker::spawn(pool.clone(), token_cache);
    backend_check_worker::spawn(pool.clone());
    archive_worker::spawn(pool, state.attachments.clone());

    // Baseline HSTS / X-Content-Type-Options / X-Frame-Options /
    // Referrer-Policy on every response. Wrapping at the outermost
    // point catches every route, including the ones added by nested
    // Routers inside `handlers::router`.
    let app = handlers::router(state).layer(axum::middleware::from_fn(
        security_headers::add_baseline_headers,
    ));

    let listener = TcpListener::bind(&bind).await.context("bind")?;
    info!(%bind, "ready");
    axum::serve(listener, app).await.context("serve")?;
    Ok(())
}

fn init_tracing() {
    // RUST_LOG-style filter, compact single-line format to
    // stdout — docker logs / journald pick it up as-is.
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "info,sqlx=warn".to_string());
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(filter))
        .init();
}

async fn run_migrations(pool: &PgPool) -> anyhow::Result<()> {
    // sqlx::migrate! resolves at compile time, embedding
    // the SQL into the binary so no on-disk migrations dir
    // is needed at runtime.
    // core/migrations is the single source of truth (0001-0030).
    // self-hosted/migrations was a byte-identical copy of 0001-0015
    // that drifted (never gained 0016+) — removed 2026-07-20; sqlx
    // checksums match so existing DBs continue at the next version.
    sqlx::migrate!("../../core/migrations").run(pool).await?;
    Ok(())
}

fn _ensure_axum_used() {
    // Suppress unused warnings when the binary is built
    // with skinny features — Router is constructed in
    // `handlers::router`. Pattern lifted from legacy.
    let _ = Router::<()>::new();
}
