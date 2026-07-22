//! # `sentori-issue-store` — issue read-side + operator triage
//!
//! Steel-tier (钢筋) crate #5. Pairs with K4
//! [`sentori_event_pipeline`]: K4 owns ingest-time writes to
//! the `issues` table; K5 owns dashboard reads + operator
//! mutations (resolve / ignore / assign / priority / labels /
//! merge / bulk-patch).
//!
//! Owns migration `core/migrations/0004_issue_triage.sql` —
//! adds `assignee_user_id` / `priority` / `labels` /
//! `resolved_in_release` columns to the K4 `issues` table
//! plus the indexes that drive operator filters.
//!
//! ## One handle, two surfaces
//!
//! ```text
//! IssueStore::new(pool)
//!   ├── .list(project_id, filter, page)  → cursor-paged issues
//!   ├── .detail(issue_id)                → IssueDetail + affected_users
//!   ├── .related(issue_id)               → cross-release "did the bug come back?" panel
//!   ├── .releases_for_issue(issue_id)    → distinct releases
//!   ├── .events_for_issue(issue_id, page)→ cursor-paged events
//!   │
//!   ├── .patch(issue_id, patch)          → status / assignee / priority / labels
//!   ├── .bulk_patch(ids, patch)          → multi-issue triage flip (single tx)
//!   └── .merge(src_id, dst_id)           → move events from src → dst, delete src
//! ```
//!
//! ## Cursor pagination
//!
//! Issues + events both keyset-paginate on
//! `(last_seen DESC, id DESC)` / `(timestamp DESC, id DESC)`.
//! See [`Cursor`] — opaque base64 the consumer round-trips.
//!
//! ## What's NOT here
//!
//! - **HTTP / axum.** The saas / self-hosted server crates
//!   wire `IssueStore` into route handlers.
//! - **Ingest writes** (K4 event-pipeline owns those).
//! - **Activity log / audit** for triage actions (K13
//!   audit-event will wrap patch/merge calls with the audit
//!   row write).
//! - **Notifier hooks** (K11 notifier reads the
//!   [`PatchOutcome`] from `patch` and decides what to
//!   enqueue).
//! - **Full-text search.** Search filter is SQL `ILIKE` over
//!   `error_type + message_sample` — sufficient for v0.1
//!   single-tenant volumes. pg_trgm / GIN search upgrade is
//!   a follow-up.
//!
//! ## Quick start
//!
//! ```no_run
//! use sentori_issue_store::{IssueStore, ListFilter, Cursor};
//! use sentori_workspace_identity::ProjectId;
//! use sqlx::PgPool;
//!
//! # async fn demo(pool: PgPool, project_id: ProjectId) -> Result<(), Box<dyn std::error::Error>> {
//! let store = IssueStore::new(pool);
//! let page = store
//!     .list(project_id, ListFilter::default(), Cursor::start(50))
//!     .await?;
//! println!("{} issues, next cursor: {:?}", page.items.len(), page.next);
//! # Ok(()) }
//! ```

#![cfg_attr(docsrs, feature(doc_cfg))]
#![allow(
    clippy::doc_markdown,
    // pub(crate)/pub(super) in private modules is load-bearing
    // for visibility from sibling modules; clippy's lint
    // assumes wider re-export and over-fires.
    clippy::redundant_pub_crate,
    // Cursor lookahead trim + last_item paths use `expect`
    // guarded by an `if items.len() > limit` check.
    clippy::missing_panics_doc,
    // i64 ↔ usize casts on bounded values (limit ≤ 500, page
    // counts) don't produce real lossy/wrap conditions.
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    // `.last().expect(...)` is guarded by `if items.len() >
    // cursor.limit` 2 lines up; the panic message is the
    // documentation.
    clippy::expect_used
)]

mod cursor;
mod error;
mod model;
mod store;

pub use cursor::{
    CURSOR_DEFAULT_LIMIT, CURSOR_MAX_LIMIT, CURSOR_MIN_LIMIT, Cursor, CursorParseError,
};
pub use error::IssueStoreError;
pub use model::{
    AffectedUsers, EventCursor, IssueCursor, IssueDetail, IssuePatch, IssueSummary, ListFilter,
    MergeOutcome, PaginatedEvents, PaginatedIssues, PatchOutcome, RelatedIssue, RelationReason,
};
pub use store::IssueStore;
