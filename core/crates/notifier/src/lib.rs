//! # `sentori-notifier` — generic outbound notification transport
//!
//! Steel-tier (钢筋) crate #11. Owns the [`Notifier`] trait,
//! three concrete transports (`Email` / `Webhook` / `Mock`), a
//! [`NotifierService`] that dispatches over a transport
//! registry, and the `delivery_log` table.
//!
//! ## Scope split with K12
//!
//! - **K11 (this crate)** — *transport* layer. Generic
//!   `Notification { channel, recipient, subject, body,
//!   metadata }`. Knows how to push bytes over SMTP /
//!   webhook. Persists every attempt in `delivery_log` with
//!   dedup + retry semantics.
//! - **K12 `integration-traits` (next ship)** — *vendor
//!   adapter* layer. Slack Block Kit, Linear GraphQL, Jira
//!   issue JSON, GitHub PR, GitLab MR. Each adapter shapes
//!   the vendor-specific payload then hands a
//!   [`Notification`] to K11's [`NotifierService`].
//!
//! ## One service handle
//!
//! ```text
//! NotifierService::new(pool)
//!   ├── register(transport)            // Email / Webhook / Mock / future
//!   │
//!   ├── dispatch(notification)         // route + persist log + return result
//!   ├── retry_one(log_id)              // re-dispatch a failed entry
//!   │
//!   ├── list_recent(project, since)    // dashboard inbox
//!   ├── list_pending(within)           // caller cron uses this
//!   └── find(log_id)                   // one entry lookup
//! ```
//!
//! ## dedup semantics
//!
//! Caller supplies a `dedup_key` on [`Notification`] (or
//! `None`). When set, a partial UNIQUE index on the
//! delivery_log table guarantees only one row exists for
//! that key — repeated `dispatch` calls with the same key
//! short-circuit and return
//! [`DispatchOutcome::Deduplicated`] without touching the
//! transport. Namespacing convention is caller's: e.g.
//! `quota-warn-{org}-{date}-90` /
//! `regression-{issue_id}-{event_ts}` /
//! `digest-{user_id}-{period}`.
//!
//! ## retry semantics
//!
//! [`NotifierService::dispatch`] is one-shot — it tries the
//! transport once + records the outcome. The retry path is
//! [`NotifierService::retry_one`]: it loads the log row,
//! increments `retries`, dispatches again (same content +
//! recipient), and updates the row in place. Caller's cron
//! `tokio::spawn` decides which failed rows to retry +
//! how often.
//!
//! ## Caller-owned cron + transport registry
//!
//! K11 doesn't spawn background work — per K7-K10
//! consistency, consumer crate (saas/server, self-hosted/
//! server) registers transports at boot and `tokio::spawn`s
//! whatever retry / digest cron it wants.
//!
//! ```no_run
//! use sentori_notifier::{
//!     NotifierService, EmailTransport, EmailConfig, SmtpTls,
//!     WebhookTransport, Channel,
//! };
//! use std::sync::Arc;
//!
//! # async fn demo(pool: sqlx::PgPool) -> Result<(), Box<dyn std::error::Error>> {
//! let mut svc = NotifierService::new(pool);
//! svc.register(Arc::new(EmailTransport::new(EmailConfig {
//!     smtp_host: "smtp.example.com".into(),
//!     smtp_port: 587,
//!     smtp_user: Some("apikey".into()),
//!     smtp_pass: Some("secret".into()),
//!     from: "sentori@example.com".into(),
//!     tls: SmtpTls::Starttls,
//! })?));
//! svc.register(Arc::new(WebhookTransport::new()));
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
    clippy::expect_used,
    clippy::missing_const_for_fn,
    clippy::derive_partial_eq_without_eq,
    clippy::single_match_else,
    clippy::match_wildcard_for_single_variants,
    clippy::too_long_first_doc_paragraph,
    clippy::option_if_let_else,
    clippy::manual_let_else,
    clippy::single_match
)]

mod error;
mod model;
mod service;
mod transports;

pub use error::{NotifierError, TransportError};
pub use model::{
    Channel, ChannelParseError, DeliveryLog, DeliveryStatus, DispatchOutcome, Notification,
};
pub use service::NotifierService;
pub use transports::{
    EmailConfig, EmailTransport, MockInbox, MockTransport, Notifier, SmtpTls, WebhookTransport,
};
