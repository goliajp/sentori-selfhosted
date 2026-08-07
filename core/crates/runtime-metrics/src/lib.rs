//! # `sentori-runtime-metrics` — time-series metrics with cascading rollups
//!
//! Steel-tier (钢筋) crate #9. Per-day partitioned raw store,
//! 3 cascading flat rollup tiers (1m / 1h / 1d), and a daily
//! [`PartitionLifecycle`] sub-handle (copy of K6's pattern
//! adapted to day grain).
//!
//! ## Tables
//!
//! - **`runtime_metrics_raw`** — RANGE-partitioned daily, 90d
//!   retention default. PK
//!   `(project_id, ts, name, tags_hash)` is idempotent across
//!   duplicate batches.
//! - **`runtime_metrics_1m / _1h / _1d`** — flat aggregates;
//!   pre-computed (count, sum, avg, p50, p95, p99) per
//!   `(project, bucket_ts, name, release, environment,
//!   device_class)`. UPSERT-idempotent rollup window re-runs.
//! - **`runtime_metrics_dropped`** — per-day-per-reason
//!   counters for ops-grade "drops happening?" badges.
//!
//! ## Pipeline
//!
//! ```text
//! SDK batch              ┌──────────────────────────┐
//!   ─────────────────►   │ ingest_batch:            │
//!                        │   canonical-JSON tags    │
//!                        │   → tags_hash            │
//!                        │   → COPY into raw        │
//!                        └──────────┬───────────────┘
//!                                   │
//!                                   ▼
//!   ┌───────────────┐   ┌──────────────────────────┐
//!   │ roll_raw_to_1m│ ◄─┤  caller's tokio::spawn   │
//!   │  (60s window) │   │  (60s tick)              │
//!   └───────┬───────┘   └──────────────────────────┘
//!           │
//!           ▼
//!   ┌───────────────┐   ┌──────────────────────────┐
//!   │ roll_1m_to_1h │ ◄─┤  caller's tokio::spawn   │
//!   │  (1h bucket)  │   │  (minute=03 hourly)      │
//!   └───────┬───────┘   └──────────────────────────┘
//!           │
//!           ▼
//!   ┌───────────────┐   ┌──────────────────────────┐
//!   │ roll_1h_to_1d │ ◄─┤  caller's tokio::spawn   │
//!   │  (1d bucket)  │   │  (03:30 UTC daily)       │
//!   └───────────────┘   └──────────────────────────┘
//! ```
//!
//! ## Cascading rollup rationale
//!
//! Each tier UPSERTs from the layer below: 1m from raw, 1h
//! from 1m, 1d from 1h. Two properties fall out:
//!
//! - **Bounded compute per tick**. The 60s 1m roll scans
//!   ~60s of raw rows; the hourly 1h roll scans ~60 1m rows
//!   per `(project, name, dims)`; the daily 1d roll scans
//!   ~24 1h rows. No tier scans raw twice.
//! - **Skippable on query**. Dashboard reading the last 30d
//!   reads `runtime_metrics_1d` (30 rows / key); reading the
//!   last day reads `_1h` (24 rows); reading the last hour
//!   reads `_1m` (60 rows). raw is only read for "last 60s
//!   live tail" UIs.
//!
//! ## Partition lifecycle — K9's copy of K6's pattern
//!
//! [`PartitionLifecycle`] manages the daily child tables of
//! `runtime_metrics_raw` (`runtime_metrics_raw_YYYY_MM_DD`).
//! The shape mirrors K6's `sentori-span-store::PartitionLifecycle`
//! with these specifics:
//!
//! - **Day grain** (vs K6's month) — partition names are
//!   `<parent>_YYYY_MM_DD` instead of `<parent>_YYYY_MM`.
//! - **90d retention default** (vs K6's 14d).
//! - **3-day forward window default** (vs K6's 6-month) —
//!   late-arriving SDK batches need a few days of pre-created
//!   partitions to land in.
//!
//! Per K9 design lock 2026-06-20, K6's lifecycle isn't
//! refactored yet. The shared `partition-lifecycle` stone
//! extraction is deferred to when retro-K4 events partition
//! lands (3-4 consumers = clear extract trigger).
//!
//! ## Quick start
//!
//! ```no_run
//! use sentori_runtime_metrics::{MetricsStore, MetricPoint};
//! use sentori_workspace_identity::ProjectId;
//! use sqlx::PgPool;
//!
//! # async fn demo(pool: PgPool, project_id: ProjectId) -> Result<(), Box<dyn std::error::Error>> {
//! let store = MetricsStore::new(pool);
//! let now = time::OffsetDateTime::now_utc();
//! let points = vec![
//!     MetricPoint::new(project_id, "app.startup_ms", now, 142.0)
//!         .with_release("myapp@5.3.1")
//!         .with_environment("production"),
//! ];
//! let written = store.ingest_batch(&points).await?;
//! assert_eq!(written, 1);
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
mod partitions;
mod store;

pub use error::RuntimeMetricsError;
pub use model::{DropReason, DropReasonParseError, DroppedRow, MetricPoint, RollupRow, RollupTier};
pub use partitions::PartitionLifecycle;
pub use store::{MetricsStore, cadence};
