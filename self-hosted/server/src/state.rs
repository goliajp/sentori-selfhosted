//! Shared axum app state — handles to every K-tier service
//! the handlers compose.
//!
//! Self-hosted is single-workspace; AppState carries the
//! constant `DEFAULT_WORKSPACE_ID` and every workspace-bound
//! service is constructed scoped to it.

use sqlx::PgPool;

use sentori_alert_rule::AlertRuleService;
use sentori_attachment_store::MemoryBlobStore;

use crate::blob_store::AttachmentStore;

/// One row of the broadcast bus — minimal so the channel stays
/// cheap to clone per fanout.
#[derive(Clone, Debug)]
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
use sentori_audit_event::AuditService;
use sentori_billing::BillingService;
use sentori_event_pipeline::{IngestOptions, IngestService};
use sentori_issue_store::IssueStore;
use sentori_push_provider::DeviceTokenStore;
use sentori_replay_store::ReplayStore;
use sentori_runtime_metrics::MetricsStore;
use sentori_saved_view::SavedViewService;
use sentori_span_store::SpanStore;
use sentori_workspace_identity::{Identity, WorkspaceId};

/// One-shot app state. All K services share the same
/// `PgPool` (sqlx pool itself is `Arc`-internally). Cheap
/// to clone.
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub workspace_id: WorkspaceId,
    pub identity: Identity,
    pub ingest: IngestService,
    pub issues: IssueStore,
    pub spans: SpanStore,
    pub replays: ReplayStore<MemoryBlobStore>,
    pub metrics: MetricsStore,
    /// Identity scope salts, keyed by workspace. Salts never change —
    /// rotating one would orphan every fingerprint written under it —
    /// so this grows to one small entry per workspace and stays there.
    pub identity_scopes: crate::identity_link::SharedScopeCache,
    /// Parsed source maps, keyed by content hash. Shared so one parse
    /// serves every event of a crashing release.
    pub source_maps: std::sync::Arc<crate::symbolicate::MapCache>,
    /// Per-token request limiter for the ingest surface. In-memory and
    /// per-process, which is right for a single instance and would
    /// need a shared store before running more than one.
    pub rate_limit: std::sync::Arc<crate::rate_limit::RateLimiter>,
    /// Per-IP limiter for the auth surface. Separate from the ingest
    /// limiter so tuning one does not affect the other, and so a
    /// brute-force response can be loud (block for minutes) without
    /// throttling legitimate SDK bursts.
    pub auth_rate_limit: std::sync::Arc<crate::rate_limit::RateLimiter>,
    pub audit: AuditService,
    pub alerts: AlertRuleService,
    pub saved_views: SavedViewService,
    pub billing: BillingService,
    pub push_tokens: DeviceTokenStore,
    /// Shared blob store for event_attachments (replay /
    /// screenshot / sourcemap / dsym / proguard). Phase D uses
    /// MemoryBlobStore; Phase E swaps to LocalFsBlobStore.
    pub attachments: AttachmentStore,
    /// Broadcast channel for live event tail (events_recent SSE).
    /// Capacity 512 — slow subscribers drop oldest, not the
    /// fast ones.
    pub events_bus: tokio::sync::broadcast::Sender<RecentEventTick>,
    /// Transactional auth email sender (verify / reset links).
    pub mailer: crate::mailer::Mailer,
    /// Env-driven Stripe config (keys + price ids + public URL).
    /// Absent keys disable the corresponding self-serve billing
    /// path rather than erroring at boot.
    pub stripe: crate::stripe::StripeConfig,
}

impl AppState {
    /// Build every K service against the shared pool + the
    /// given workspace scope.
    ///
    /// # Panics
    ///
    /// `IngestService::new` returns `Result` for the
    /// fingerprint hasher init; v0.1 panics on the
    /// vanishingly unlikely failure since it's a process-
    /// boot wiring step.
    #[must_use]
    pub fn new(pool: PgPool, workspace_id: WorkspaceId, attachments: AttachmentStore) -> Self {
        let (events_bus, _) = tokio::sync::broadcast::channel(512);
        let identity = Identity::new(pool.clone(), workspace_id);
        // Boot-time wiring: the fingerprint hasher init is the only
        // fallible part and a failure means the process cannot serve
        // ingest at all, so there is nothing to degrade to.
        #[allow(clippy::expect_used)]
        let ingest = IngestService::new(pool.clone(), IngestOptions::default())
            .expect("ingest service must build");
        let issues = IssueStore::new(pool.clone());
        let spans = SpanStore::new(pool.clone());
        let replays = ReplayStore::new(
            pool.clone(),
            MemoryBlobStore::new(),
            sentori_replay_store::Scrubber::owasp_default(),
        );
        let metrics = MetricsStore::new(pool.clone());
        let audit = AuditService::new(pool.clone());
        let alerts = AlertRuleService::new(pool.clone());
        let saved_views = SavedViewService::new(pool.clone());
        let billing = BillingService::new(pool.clone(), workspace_id);
        let push_tokens = DeviceTokenStore::new(pool.clone());
        let mailer = crate::mailer::Mailer::from_env();
        let stripe = crate::stripe::StripeConfig::from_env();
        Self {
            pool,
            workspace_id,
            identity,
            ingest,
            issues,
            spans,
            replays,
            metrics,
            identity_scopes: std::sync::Arc::default(),
            source_maps: std::sync::Arc::new(crate::symbolicate::new_cache()),
            rate_limit: std::sync::Arc::new(crate::rate_limit::RateLimiter::from_env()),
            auth_rate_limit: std::sync::Arc::new(crate::rate_limit::RateLimiter::auth_from_env()),
            audit,
            alerts,
            saved_views,
            billing,
            push_tokens,
            attachments,
            events_bus,
            mailer,
            stripe,
        }
    }

    /// An [`Identity`] handle scoped to a specific workspace —
    /// typically `ctx.workspace_id` from the session middleware.
    ///
    /// `self.identity` is bound to the boot-time default workspace
    /// and must NOT be used for request-scoped work: in a
    /// multi-tenant (SaaS) deployment every authenticated request
    /// acts in its caller's active workspace, not the default one.
    /// `Identity::new` is a cheap pool clone + a copied id, so
    /// building one per request is fine.
    #[must_use]
    pub fn identity_for(&self, workspace_id: WorkspaceId) -> Identity {
        Identity::new(self.pool.clone(), workspace_id)
    }

    /// A [`BillingService`] scoped to a specific workspace. Same
    /// rationale as [`Self::identity_for`]: `self.billing` is bound
    /// to the default workspace and is wrong for request-scoped
    /// quota / plan lookups.
    #[must_use]
    pub fn billing_for(&self, workspace_id: WorkspaceId) -> BillingService {
        BillingService::new(self.pool.clone(), workspace_id)
    }
}
