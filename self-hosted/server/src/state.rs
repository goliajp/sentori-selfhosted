//! Shared axum app state.
//!
//! v1 keeps this deliberately thin: handlers talk to Postgres with
//! sqlx directly (cement-first — the ingest pipeline and issue
//! logic live in server modules and get promoted to crates when a
//! second consumer appears, not before). What lives here is only
//! what must be shared and long-lived: the pool, caches, limiters,
//! and the transports.

use sqlx::PgPool;

use crate::blob_store::AttachmentStore;

/// One row of the broadcast bus — minimal so the channel stays
/// cheap to clone per fanout.
#[derive(Clone, Debug)]
// Read by future bus subscribers (live tail); today only constructed.
#[allow(dead_code)]
pub struct RecentEventTick {
    pub project_id: uuid::Uuid,
    pub issue_id: uuid::Uuid,
    pub event_id: uuid::Uuid,
    pub kind: String,
    pub release: String,
    pub environment: String,
    pub platform: String,
    pub timestamp: time::OffsetDateTime,
}

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    /// Parsed source maps, keyed by content hash. Shared so one parse
    /// serves every event of a crashing release.
    pub source_maps: std::sync::Arc<crate::symbolicate::MapCache>,
    /// Per-token request limiter for the ingest surface. In-memory and
    /// per-process, which is right for a single instance.
    pub rate_limit: std::sync::Arc<crate::rate_limit::RateLimiter>,
    /// Per-IP limiter for the auth surface. Separate from the ingest
    /// limiter so a brute-force response can be loud without
    /// throttling legitimate SDK bursts.
    pub auth_rate_limit: std::sync::Arc<crate::rate_limit::RateLimiter>,
    /// Content-addressed blob store for event attachments and
    /// symbolication artifacts.
    pub attachments: AttachmentStore,
    /// Broadcast channel for a live event tail. Capacity 512 — slow
    /// subscribers drop oldest, not the fast ones.
    pub events_bus: tokio::sync::broadcast::Sender<RecentEventTick>,
    /// Transactional email sender (password reset, issue notify).
    pub mailer: crate::mailer::Mailer,
    /// Push device-token store (push family is carried, not part of
    /// the v1 acceptance surface).
    pub push_tokens: sentori_push_provider::DeviceTokenStore,
}

impl AppState {
    #[must_use]
    pub fn new(pool: PgPool, attachments: AttachmentStore) -> Self {
        let (events_bus, _) = tokio::sync::broadcast::channel(512);
        let pool_for_push = pool.clone();
        Self {
            pool,
            source_maps: std::sync::Arc::new(crate::symbolicate::new_cache()),
            rate_limit: std::sync::Arc::new(crate::rate_limit::RateLimiter::from_env()),
            auth_rate_limit: std::sync::Arc::new(crate::rate_limit::RateLimiter::auth_from_env()),
            attachments,
            events_bus,
            mailer: crate::mailer::Mailer::from_env(),
            push_tokens: sentori_push_provider::DeviceTokenStore::new(pool_for_push),
        }
    }
}
