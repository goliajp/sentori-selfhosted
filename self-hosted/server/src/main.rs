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
mod apps;
mod archive_worker;
mod audience;
mod audit;
mod backend_check_worker;
mod backfill_split;
mod blob_store;
mod bootstrap;
mod bundle;
mod client_ip;
mod cloud;
mod cloud_quota;
mod credential_vault;
mod db;
mod env_config;
mod event_filter;
mod fcm;
mod handlers;
mod hcm;
mod identity;
mod ingest_metrics;
mod instance_operations;
mod integrations;
mod jobs;
mod mailer;
mod mipush;
mod monitors;
mod native_symbolicate;
mod notification_delivery;
mod notification_queue;
mod pipeline;
mod project_transfers;
mod public_http;
mod push_credential_check;
mod push_credential_probe;
mod push_quarantine;
mod push_secrets;
mod push_worker;
mod rate_limit;
mod resymbolicate;
mod security_headers;
mod session_mw;
mod source_bundle;
mod state;
mod symbolicate;
mod team;
mod token_cache;
mod transfer_tail;
mod verify_artifacts;
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
    let pool = db::connect(&db_url).await.context("db connect")?;
    let control_plane: bool =
        sqlx::query_scalar("SELECT to_regclass('public.cloud_accounts') IS NOT NULL")
            .fetch_one(&pool)
            .await?;
    anyhow::ensure!(
        !control_plane,
        "Data plane requires a separate database; Cloud control-plane tables detected"
    );
    let mut instance_lock = sentori_snapshot::reset::shared_guard(&pool).await?;
    sentori_snapshot::reset::ensure_available(&pool).await?;

    // `sentori-server reset-password <email>` — the operator path
    // for a locked-out owner: no SMTP dependency,
    // prints a fresh password to stdout and exits.
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("reset-password") {
        anyhow::ensure!(
            !cloud::bound(&pool).await?
                && env_config::env_or_file("SENTORI_CLOUD_WORKSPACE_ID").is_none(),
            "Cloud passwords are managed by the control plane"
        );
        let email = args
            .get(2)
            .context("usage: sentori-server reset-password <email>")?;
        return bootstrap::reset_password(&pool, email).await;
    }

    // `sentori-server backfill-split` — one-shot: split pre-2.9.0
    // mixed issues by environment × platform (see backfill_split.rs).
    if args.get(1).map(String::as_str) == Some("backfill-split") {
        return backfill_split::run(&db_url)
            .await
            .map_err(|e| anyhow::anyhow!("backfill-split: {e}"));
    }

    // `sentori-server resymbolicate [<release>]` — re-read stored
    // crashes against the artifacts now on hand. Uploads trigger this
    // themselves; the subcommand covers what they cannot (an upload
    // that predates the trigger, a pass that failed, a replaced map).
    if args.get(1).map(String::as_str) == Some("resymbolicate") {
        return resymbolicate::run(&db_url, args.get(2).map(String::as_str))
            .await
            .map_err(|e| anyhow::anyhow!("resymbolicate: {e}"));
    }

    // `sentori-server verify-artifacts [--all]` — read the artifacts
    // stored before the upload-time check existed and record what
    // they are.
    if args.get(1).map(String::as_str) == Some("verify-artifacts") {
        let all = args.iter().any(|a| a == "--all");
        return verify_artifacts::run(&db_url, all)
            .await
            .map_err(|e| anyhow::anyhow!("verify-artifacts: {e}"));
    }

    let bind = std::env::var("SENTORI_BIND").unwrap_or_else(|_| "0.0.0.0:8080".to_string());

    info!(%bind, "sentori self-hosted server boot");

    run_migrations(&pool).await.context("migrate")?;
    sentori_snapshot::reset::ensure_available(&pool).await?;
    let cloud = cloud::configure(&pool).await?;

    // Env-declared owner reconcile (idempotent, declarative).
    if cloud.is_none()
        && let Err(e) = bootstrap::ensure_owner(&pool).await
    {
        tracing::warn!(error = %e, "owner bootstrap skipped");
    }

    let attachments = blob_store::AttachmentStore::from_env()
        .await
        .context("attachment store init")?;
    anyhow::ensure!(
        cloud.is_none() || matches!(attachments, blob_store::AttachmentStore::Fs(_)),
        "Cloud requires a persistent filesystem attachment store"
    );
    let mut state = AppState::new(pool.clone(), attachments);
    state.cloud = cloud;
    let state = Arc::new(state);

    // Recovery verification must not resend old work or prune restored evidence.
    let operating_mode: String =
        sqlx::query_scalar("SELECT operating_mode FROM instance_settings WHERE singleton")
            .fetch_one(&pool)
            .await?;
    cloud_quota::spawn(Arc::clone(&state), operating_mode == "normal");
    if operating_mode == "normal" {
        let token_cache = std::sync::Arc::new(token_cache::TokenCache::new());
        instance_operations::spawn(Arc::clone(&state));
        project_transfers::spawn(Arc::clone(&state));
        jobs::spawn(Arc::clone(&state));
        team::spawn(state.pool.clone());
        push_secrets::spawn(Arc::clone(&state));
        push_worker::spawn(Arc::clone(&state), token_cache);
        backend_check_worker::spawn(pool.clone());
        monitors::spawn(pool.clone());
        notification_delivery::spawn(&state);
        archive_worker::spawn(pool, state.attachments.clone(), state.cloud.is_some());
    } else {
        tracing::warn!(%operating_mode, "automatic workers paused for recovery validation");
    }

    // Baseline HSTS / X-Content-Type-Options / X-Frame-Options /
    // Referrer-Policy on every response. Wrapping at the outermost
    // point catches every route, including the ones added by nested
    // Routers inside `handlers::router`.
    let app = handlers::router(state).layer(axum::middleware::from_fn(
        security_headers::add_baseline_headers,
    ));

    let listener = TcpListener::bind(&bind).await.context("bind")?;
    info!(%bind, "ready");
    tokio::select! {
        result = axum::serve(listener, app) => result.context("serve")?,
        result = async {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                sqlx::query("SELECT 1").execute(&mut instance_lock).await.context("instance lock connection lost; stopping server")?;
            }
            #[allow(unreachable_code)] Ok::<(),anyhow::Error>(())
        } => result?,
    }
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
