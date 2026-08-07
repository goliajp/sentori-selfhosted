//! # `sentori-integration-traits` — issue lifecycle → upstream vendor item
//!
//! Steel-tier (钢筋) crate #12. Provides the
//! [`IntegrationAdapter`] trait and an
//! [`IntegrationService`] dispatcher that:
//!
//! 1. Stores per-project adapter config (`integrations`).
//! 2. Fans Sentori issue lifecycle events (Created /
//!    Regressed / Resolved) out to each connected adapter.
//! 3. Persists upstream item refs in
//!    `issue_integration_links` so subsequent updates target
//!    the right thread / ticket / PR.
//!
//! ## What ships in K12 vs K12.1-K12.4
//!
//! - **K12 (this crate)** — trait + service + 2 tables +
//!   [`SlackAdapter`] reference impl + [`MockAdapter`] for
//!   tests. Slack is `ConnectMode::Manual` (incoming
//!   webhook URL — no OAuth flow), making it the smallest
//!   complete adapter and a clean trait validator.
//! - **K12.1 Linear** — OAuth + GraphQL.
//! - **K12.2 Jira** — OAuth (Atlassian 3LO) + REST.
//! - **K12.3 GitHub** — OAuth (or App) + REST.
//! - **K12.4 GitLab** — OAuth + REST.
//!
//! Each vendor follow-up implements [`IntegrationAdapter`]
//! and registers via [`IntegrationService::register`]; no
//! schema change needed (the DB `kind` column is `TEXT`).
//!
//! ## How K12 composes with K11
//!
//! K11 `sentori_notifier::WebhookTransport` is a *generic*
//! HTTP POST primitive. K12 adapters call **vendor-specific
//! APIs** (Linear GraphQL, Slack incoming webhook with Block
//! Kit shape, Jira REST PATCH on status, …). They're
//! siblings, not composed:
//!
//! - K11 NotifierService → "alert this human via email /
//!   plain webhook".
//! - K12 IntegrationService → "open a Linear ticket for
//!   this Sentori issue, then close it when the issue
//!   resolves".
//!
//! Both crates are caller-owned (no background tasks).
//!
//! ## Trait surface
//!
//! ```ignore
//! #[async_trait]
//! pub trait IntegrationAdapter {
//!     fn kind(&self) -> &'static str;        // "slack" / "linear" / …
//!     fn is_configured(&self) -> bool;       // env vars present?
//!     fn connect_mode(&self) -> ConnectMode; // OAuth / Manual
//!     fn oauth_authorise_url(&self, state, redirect_uri) -> String;
//!     async fn exchange_code(&self, code, redirect_uri) -> Result<Value, _>;
//!     async fn accept_manual_config(&self, form) -> Result<Value, _>;
//!     async fn create_issue(&self, config, ctx) -> Result<ExternalRef, _>;
//!     async fn update_status(&self, config, external_id, event) -> Result<(), _>;
//! }
//! ```
//!
//! The `config: &Value` parameter on `create_issue` /
//! `update_status` is the JSONB blob stored in
//! `integrations.config` — adapter knows how to read it.
//!
//! ## Caller-owned dispatch + storage CRUD
//!
//! ```no_run
//! use sentori_integration_traits::{
//!     IntegrationService, IssueContext, IssueLifecycleEvent,
//!     SlackAdapter,
//! };
//! use std::sync::Arc;
//!
//! # async fn demo(pool: sqlx::PgPool, ctx: IssueContext) -> Result<(), Box<dyn std::error::Error>> {
//! let mut svc = IntegrationService::new(pool);
//! svc.register(Arc::new(SlackAdapter::new()));
//! // After dashboard form submission → `svc.store_config(...)`.
//! // On new issue:
//! let _outcome = svc.dispatch(&ctx, IssueLifecycleEvent::Created).await?;
//! # Ok(()) }
//! ```

#![cfg_attr(docsrs, feature(doc_cfg))]
#![allow(
    clippy::doc_markdown,
    clippy::redundant_pub_crate,
    clippy::missing_panics_doc,
    clippy::missing_const_for_fn,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::expect_used,
    clippy::derive_partial_eq_without_eq,
    clippy::option_if_let_else,
    clippy::manual_let_else,
    clippy::single_match,
    clippy::too_long_first_doc_paragraph,
    clippy::similar_names,
    clippy::map_unwrap_or,
    clippy::redundant_clone
)]

pub mod adapters;
mod error;
mod model;
mod service;
mod traits;

pub use adapters::mock::{MockAdapter, MockFailMode, MockHistory, RecordedCall};
pub use adapters::slack::{SlackAdapter, SlackConfig};
pub use error::IntegrationError;
pub use model::{
    ConnectMode, DispatchOutcome, ExternalRef, IntegrationConfig, IssueContext,
    IssueIntegrationLink, IssueLifecycleEvent,
};
pub use service::IntegrationService;
pub use traits::IntegrationAdapter;
