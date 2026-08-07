//! # `sentori-span-store` — distributed tracing storage + partition lifecycle
//!
//! Steel-tier (钢筋) crate #6.
//!
//! Owns two tables (`core/migrations/0005_span_pipeline.sql`):
//!
//! - **`spans`** — RANGE-partitioned monthly by `received_at`,
//!   PK `(received_at, id)` so the partition key is part of
//!   the uniqueness contract. Indexed by trace_id, parent_span_id
//!   (partial — root spans have NULL parent), `(project_id,
//!   received_at)` for trace-list paging, `(project_id, op)`
//!   for span-by-op search.
//! - **`traces`** — UPSERT-keyed rollup, NOT partitioned (a
//!   partitioned table's unique index must include the
//!   partition key, which kills `ON CONFLICT (trace_id)`).
//!   Carries root-span fields (root_op / root_name /
//!   duration_ms) populated when the root span lands; child
//!   spans bump span_count, refresh last_seen, and promote
//!   status to worst-of (error > cancelled > ok).
//!
//! ## One handle, two surfaces
//!
//! ```text
//! SpanStore::new(pool)
//!   ├── ingest_span(project_id, span)         → single-tx INSERT span + UPSERT trace
//!   │
//!   ├── list_traces(project_id, filter, page) → cursor-paged trace summaries
//!   ├── trace_detail(trace_id)                → trace + full waterfall
//!   ├── spans_for_trace(trace_id)             → flat span list, ordered by started_at
//!   │
//!   └── .partitions()                         → PartitionLifecycle
//!         ├── ensure_future(months_ahead)     → create N months forward
//!         ├── drop_before(cutoff)             → drop partitions wholly before cutoff
//!         ├── prune_traces_before(cutoff)     → DELETE WHERE last_seen < cutoff
//!         └── prune_orphan_traces(now, grace) → DELETE root_op IS NULL after grace
//! ```
//!
//! ## Partition lifecycle (first in v0.1)
//!
//! K6 is the first crate in v0.1 to introduce monthly RANGE
//! partitioning. The mechanics here ([`PartitionLifecycle`])
//! intentionally generalise so K9 runtime-metrics and the
//! K4-events follow-up can reuse them. The relevant pieces:
//!
//! - **Bootstrap partitions** in the migration (6 months) +
//!   a `DEFAULT` partition catching stray writes.
//! - **`ensure_future_partitions(n)`** at startup / from a
//!   daily janitor cron — `CREATE TABLE IF NOT EXISTS` per
//!   missing month, idempotent.
//! - **`drop_partitions_before(cutoff)`** — `DROP TABLE` per
//!   monthly child whose upper bound ≤ cutoff. O(1) per
//!   partition; the alternative `DELETE WHERE received_at <
//!   cutoff` is O(rows) and would hammer autovacuum.
//!
//! Once K9 / K4-followup reach for the same pattern,
//! `PartitionLifecycle` can graduate to a shared stone
//! (rule-of-three threshold).
//!
//! ## Atomic span ingest
//!
//! [`SpanStore::ingest_span`] runs one transaction:
//!
//! 1. `INSERT INTO spans (...) VALUES (...)`
//! 2. `INSERT INTO traces (trace_id, project_id, root_*, span_count, status, …)
//!    ON CONFLICT (trace_id) DO UPDATE SET
//!      span_count = traces.span_count + 1,
//!      last_seen  = GREATEST(traces.last_seen, EXCLUDED.last_seen),
//!      root_op    = COALESCE(traces.root_op,   EXCLUDED.root_op),
//!      root_name  = COALESCE(traces.root_name, EXCLUDED.root_name),
//!      duration_ms = CASE WHEN EXCLUDED.root_op IS NOT NULL
//!                         THEN EXCLUDED.duration_ms ELSE traces.duration_ms END,
//!      status     = worst_of(traces.status, EXCLUDED.status)`
//!
//! Either both writes commit or neither does — dashboard
//! never observes a span without its trace row.
//!
//! ## Cursor pagination
//!
//! [`crate::Cursor`] re-exports K5's
//! [`sentori_issue_store::Cursor`]. Same opaque base64 envelope,
//! same lookahead-by-1 walk. K6 is the second consumer (after
//! K5); if a third + fourth appears the type lifts to a shared
//! stone.

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
mod partitions;
mod store;

pub use error::SpanStoreError;
pub use model::{
    ListTraceFilter, PaginatedTraces, Span, SpanInput, SpanStatus, SpanStatusParseError, Trace,
    TraceDetail,
};
pub use partitions::PartitionLifecycle;
pub use store::SpanStore;

// Re-export K5's Cursor — same opaque envelope, same paging
// shape. Saves a duplicate impl until rule-of-three.
pub use sentori_issue_store::{Cursor, CursorParseError};
