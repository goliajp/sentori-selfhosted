//! # `sentori-push-provider` — push notification trait + dispatcher + tokens
//!
//! Steel-tier (钢筋) crate #7. Composes:
//!
//! - K1 [`sentori_workspace_identity`] for `ProjectId`.
//! - S10 [`sentori_rate_limiter`] for per-(project, provider)
//!   send-rate enforcement.
//! - S12 [`sentori_secrets_vault`] for at-rest encryption of
//!   vendor credentials (APNs p8, FCM service-account JSON,
//!   VAPID private key, …).
//!
//! Owns two tables (`core/migrations/0006_push_tokens.sql`):
//! `push_tokens` (one row per device token) and
//! `push_credentials` (one row per (project, provider) with
//! S12-sealed `secret_blob`).
//!
//! ## What ships in K7
//!
//! Per user decision 2026-06-20:
//!
//! - [`PushProvider`] async trait (dyn-dispatchable via
//!   `async-trait`).
//! - [`PushDispatcher`] composing tokens + credentials +
//!   provider registry + S10 rate-limiter + DB-level
//!   quarantine into one `dispatch(target, msg)` call.
//! - [`DeviceTokenStore`] / [`CredentialStore`].
//! - [`MockProvider`] for tests + the consumer integration
//!   suite. Returns caller-configured outcomes.
//! - Per-(project, provider) L1 rate limit. L2 per-project +
//!   L3 global-inflight tiers are follow-ups.
//! - DB-level quarantine: `PermanentlyInvalidToken` outcome
//!   stamps `push_tokens.quarantined_at`; dispatcher skips
//!   quarantined rows. Health metric (per-project per-provider
//!   bad-rate gauge) is a follow-up.
//!
//! ## What ships LATER
//!
//! - 5 real vendor impls (K7.1 APNs / K7.2 FCM HTTP v1 /
//!   K7.3 Web Push / K7.4 HCM / K7.5 MiPush). Each is a focused
//!   600-1000 LOC follow-up that implements `PushProvider`
//!   without touching K7's surface.
//! - L2 / L3 rate-limit tiers (compose more `Limiter`s).
//! - Health metric crate.
//! - HTTP/JSON route handling (lives in the saas / self-hosted
//!   server bins).
//! - Dispatch cron / scheduler (caller drives via
//!   `tokio::spawn` over [`PushDispatcher::dispatch`]).
//!
//! ## Quick start
//!
//! ```no_run
//! use std::sync::Arc;
//! use sentori_push_provider::{
//!     DispatchTarget, MockProvider, NativeMessage, ProviderKind,
//!     ProviderRegistry, PushDispatcher, RateLimits, SendOutcome,
//! };
//! use sentori_secrets_vault::{MasterKey, Vault, KeyId};
//! use sentori_workspace_identity::ProjectId;
//! use sqlx::PgPool;
//!
//! # async fn demo(pool: PgPool, project_id: ProjectId) -> Result<(), Box<dyn std::error::Error>> {
//! let mut registry = ProviderRegistry::new();
//! registry.register(ProviderKind::Apns, Arc::new(MockProvider::always(SendOutcome::Sent)));
//!
//! let vault = Vault::new(MasterKey::generate()?, KeyId::new("k1")?);
//! let dispatcher = PushDispatcher::new(pool, registry, vault, RateLimits::default());
//! let outcome = dispatcher
//!     .dispatch(DispatchTarget::ProjectKind { project_id, kind: ProviderKind::Apns },
//!               NativeMessage::simple("Hello", "Body"))
//!     .await?;
//! println!("sent={}", outcome.sent);
//! # Ok(()) }
//! ```

#![cfg_attr(docsrs, feature(doc_cfg))]
#![allow(
    clippy::doc_markdown,
    clippy::redundant_pub_crate,
    clippy::missing_panics_doc,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::expect_used
)]

mod credentials;
mod dispatcher;
mod error;
mod mock;
mod model;
mod provider;
mod registry;
mod tokens;

pub use credentials::{CredentialStore, StoredCredential};
pub use dispatcher::{
    DispatchOutcome, DispatchTarget, PerTokenOutcome, PushDispatcher, RateLimits,
};
pub use error::PushError;
pub use mock::MockProvider;
pub use model::{
    Credential, DeviceToken, MintedToken, NativeMessage, ProviderKind, ProviderKindParseError,
    ProviderResult, SendOutcome, ValidateOutcome,
};
pub use provider::PushProvider;
pub use registry::ProviderRegistry;
pub use tokens::DeviceTokenStore;
