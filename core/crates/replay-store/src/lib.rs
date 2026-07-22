//! # `sentori-replay-store` — wireframe replay storage + PII scrub
//!
//! Steel-tier (钢筋) crate #8. Composes:
//!
//! - K1 [`sentori_workspace_identity`] for `ProjectId`.
//! - K3 [`sentori_attachment_store`] for content-addressed
//!   blob persistence (SHA-256-keyed; cross-session
//!   dedup falls out for free).
//! - K4 [`sentori_event_pipeline`] for the FK to `events.id` —
//!   every replay attaches to one event.
//!
//! Owns one table (`core/migrations/0007_replay_sessions.sql`):
//! `replay_sessions`. The actual NDJSON bytes live in K3.
//!
//! ## The pipeline
//!
//! ```text
//! caller hands raw NDJSON      ┌──────────────┐    ┌────────────┐
//!         ─────────────────►   │ Scrubber     │ ─► │   gzip     │
//!                              │ (OWASP PII   │    │ (flate2,   │
//!                              │  + extras)   │    │  rust-impl)│
//!                              └──────────────┘    └─────┬──────┘
//!                                                        │
//!                                              SHA-256 ──┴──► K3
//!                              ┌──────────────────────────┐  put
//!                              │ replay_sessions row      │ ◄────
//!                              │ id / event_id / blob_hash│
//!                              │ frame_count / scrubbed   │
//!                              │ byte_count               │
//!                              └──────────────────────────┘
//! ```
//!
//! Write-time scrub is per user decision 2026-06-20: the
//! stored bytes are already redacted; an operator viewing
//! the dashboard never sees raw PII. The trade is that a
//! later regex tweak can't retroactively re-scrub historical
//! sessions; callers that need that re-fetch via
//! [`ReplayStore::fetch`], re-scrub, and store under a new
//! session id.
//!
//! ## One handle
//!
//! ```text
//! ReplayStore::new(pool, blob_store, scrubber)
//!   ├── store(project, event, ndjson)    → write-time scrub + gzip + K3 put + row
//!   ├── fetch(session_id)                → load + gunzip → raw scrubbed NDJSON
//!   ├── list_for_event(event_id)         → metadata rows (no blob load)
//!   ├── list_for_project(project, cursor)→ paginated metadata across all events
//!   ├── find(session_id)                 → one metadata row
//!   └── delete(session_id)               → drop row; blob GC is janitor's job
//! ```
//!
//! ## Quick start
//!
//! ```no_run
//! use sentori_attachment_store::MemoryBlobStore;
//! use sentori_replay_store::{ReplayStore, Scrubber};
//!
//! # async fn demo(
//! #     pool: sqlx::PgPool,
//! #     project_id: sentori_workspace_identity::ProjectId,
//! #     event_id: uuid::Uuid,
//! # ) -> Result<(), Box<dyn std::error::Error>> {
//! let store = ReplayStore::new(
//!     pool,
//!     MemoryBlobStore::new(),
//!     Scrubber::owasp_default(),
//! );
//! let ndjson = br#"{"ts":1,"kind":"key","nodes":[{"text":"email: alice@example.com"}]}"#;
//! let now = time::OffsetDateTime::now_utc();
//! let session = store.store(project_id, event_id, ndjson, now, now).await?;
//! assert!(session.scrubbed_count >= 1, "the email was redacted");
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

mod error;
mod model;
mod scrubber;
mod store;

pub use error::ReplayStoreError;
pub use model::{PaginatedReplays, ReplaySession};
pub use scrubber::{REDACTED_PLACEHOLDER, ScrubReport, Scrubber, ScrubberError};
pub use store::ReplayStore;

// Re-export K5's Cursor for the project-paginated list, per
// the K6 precedent — single opaque envelope used across the
// dashboard.
pub use sentori_issue_store::{Cursor, CursorParseError};
