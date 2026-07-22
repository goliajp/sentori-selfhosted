//! # `sentori-cert-monitor` — Certificate Transparency log poll
//!
//! Steel-tier (钢筋) crate #10. Polls public CT logs (default
//! crt.sh) for certificates issued against operator-watched
//! domains and stores the observations for dashboard alerting.
//!
//! ## Use case
//!
//! Defence against **rogue issuance**. A CA mis-issues
//! `mybank.example.com` to an attacker; the cert lands in
//! public CT logs (per Chrome + Firefox + Safari requirements);
//! the operator's K10-driven crt.sh poll surfaces it within
//! the next tick, an alert fires, the operator initiates
//! revocation.
//!
//! NOT the same as "TLS expiry watch" (connect to my :443 +
//! parse cert + warn at N days before expiry) — that feature
//! is deferred. The plan §B parenthetical was misleading; K10
//! mirrors legacy `cert_monitor.rs` which is CT log monitoring.
//!
//! ## One handle
//!
//! ```text
//! CertMonitor::new(pool)              // optional .with_base_url("https://crt.sh")
//!   ├── add_watch(project, domain, added_by) → upserts cert_watch_domains
//!   ├── list_watched(project)                → enumerate watch entries
//!   ├── remove_watch(project, domain)        → delete watch entry
//!   │
//!   ├── poll_once()                          → iterate every watch, return new observations
//!   ├── poll_domain(project, domain)         → single-domain probe
//!   │
//!   ├── list_observations(project, since)    → dashboard inbox query
//!   └── list_expiring(project, within)       → "expires in < N days" badge
//! ```
//!
//! ## Caller-owned cron
//!
//! K10 does NOT spawn a background task. Per K7-K9
//! consistency, the consumer (saas/server,
//! self-hosted/server) drives:
//!
//! ```ignore
//! let monitor = CertMonitor::new(pool);
//! tokio::spawn(async move {
//!     loop {
//!         tokio::time::sleep(Duration::from_secs(600)).await;
//!         if let Err(e) = monitor.poll_once().await {
//!             tracing::warn!(error = %e, "cert poll tick failed");
//!         }
//!     }
//! });
//! ```
//!
//! ## crt.sh wire format
//!
//! The default `base_url = https://crt.sh`. K10 hits:
//!
//! ```text
//! GET <base_url>/?q=%25.<encoded_domain>&output=json
//! ```
//!
//! Response is a JSON array of `{id, common_name, name_value,
//! issuer_name, not_before, not_after, ...}`. crt.sh returns
//! zoneless ISO 8601 (`2024-01-01T00:00:00`); K10 parses as
//! UTC. Tests inject a mock `base_url` to drive the response
//! shape deterministically.

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

mod error;
mod model;
mod monitor;

pub use error::CertMonitorError;
pub use model::{CertObservation, PollOutcome, WatchedDomain};
pub use monitor::{CertMonitor, DEFAULT_BASE_URL, DEFAULT_HTTP_TIMEOUT_SECS};
