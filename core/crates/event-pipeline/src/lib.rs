//! # `sentori-event-pipeline` — ingest validation + fingerprint + persist
//!
//! Steel-tier (钢筋) crate #4. The pure-domain core of Sentori's
//! ingest path: validates one inbound event, fingerprints it (S3),
//! UPSERTs the owning issue row with atomic regression detection,
//! and persists the event row. Composes:
//!
//! - K1 [`sentori_workspace_identity`] for `ProjectId`.
//! - S3 [`sentori_issue_fingerprint`] for the fingerprint algorithm.
//! - S4 [`sentori_event_ringbuffer`] for the bounded write buffer.
//!
//! Owns two tables (migration `core/migrations/0003_event_pipeline.sql`):
//! `issues` and `events`.
//!
//! ## What's deliberately NOT here
//!
//! - **HTTP routes.** The saas/server and self-hosted/server
//!   bins wrap the ingest endpoint around this service.
//! - **Notification dispatch.** K11 `notifier` reads issue
//!   lifecycle outcomes (new / regressed) emitted as side effects
//!   by the consumer crate after `ingest` returns.
//! - **Integration dispatch.** K12 `integration-traits` (Linear /
//!   Slack / Jira webhooks).
//! - **Alert rule evaluation.** K14 `alert-rule`.
//! - **Quota check.** K17 `billing`.
//! - **Symbolication.** Caller resolves stacks via S6 sourcemap-
//!   resolver / S7 dwarf-resolver / S8 proguard-resolver BEFORE
//!   handing the event to this service. Keeping the symbolicator
//!   out keeps K4's scope tight; the consumer crate composes
//!   the two.
//! - **Background flusher task.** This crate ships
//!   [`IngestService::try_enqueue`] + [`IngestService::flush`];
//!   the consumer owns the `tokio::spawn` flush loop. Per user
//!   decision 2026-06-20 (testability + no surprise tokio
//!   lifecycles attached to the type).
//!
//! ## One handle, three entrypoints
//!
//! ```text
//! IngestService::new(pool, opts)
//!   ├── .ingest(project_id, event)        → write-through (one DB round-trip)
//!   ├── .try_enqueue(project_id, event)   → push into ring (cheap)
//!   ├── .flush()                          → drain ring + persist each (caller-driven)
//!   ├── ::fingerprint(event)              → pure compute (no I/O)
//!   └── .ringbuffer()                     → borrow the underlying Ring for telemetry
//! ```
//!
//! ## The Event shape
//!
//! Per user decision 2026-06-20, [`Event`] is **slim typed +
//! JSONB `payload`**. Top-level fields are the ones the dashboard
//! facets / fingerprints on; everything else (device, app,
//! breadcrumbs, tags, user, geo, bundle, flags, attachments[],
//! framework, link_hashes, symbolication) lives in `payload`.
//! SDK additions are zero-migration.
//!
//! ## Regression detection (atomic)
//!
//! The issue UPSERT in [`IngestService::ingest`] uses one SQL
//! `INSERT ... ON CONFLICT DO UPDATE` that simultaneously bumps
//! `last_seen` / `event_count` AND, when the row was previously
//! `status = 'resolved'`, flips it to `'regressed'` and stamps
//! `regressed_at` + `regressed_in_release` from the incoming
//! event. There is no read-then-write window where the dashboard
//! could observe a stale `resolved` after the regression landed.
//!
//! ## Quick start
//!
//! ```no_run
//! use sentori_event_pipeline::{Event, EventKind, IngestService, IngestOptions, Platform};
//! use sentori_workspace_identity::ProjectId;
//! use sqlx::PgPool;
//!
//! # async fn demo(pool: PgPool, project_id: ProjectId) -> Result<(), Box<dyn std::error::Error>> {
//! let svc = IngestService::new(pool, IngestOptions::default())?;
//! let event = Event::exception(
//!     uuid::Uuid::now_v7(),
//!     time::OffsetDateTime::now_utc(),
//!     Platform::Ios,
//!     "myapp@5.3.1",
//!     "production",
//!     "TypeError",
//!     "Cannot read property 'id' of undefined",
//! );
//! let outcome = svc.ingest(project_id, event).await?;
//! println!("issue {} new={} regressed={}", outcome.issue_id, outcome.is_new_issue, outcome.regressed);
//! # Ok(()) }
//! ```

#![cfg_attr(docsrs, feature(doc_cfg))]
// Identifier-prose policy — same reason as K2 / K3.
#![allow(clippy::doc_markdown)]

mod error;
mod model;
mod pipeline;
mod store;

pub use error::IngestError;
pub use model::{
    EnqueuedEvent, Event, EventKind, EventKindParseError, FrameSite, IngestOutcome, Issue,
    IssueStatus, IssueStatusParseError, MessageLevel, MessageLevelParseError, Platform,
    PlatformParseError, StoredEvent,
};
pub use pipeline::{IngestOptions, IngestService};
