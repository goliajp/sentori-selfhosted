//! POST `/v1/runtime-metrics:batch` — auto-instrumented perf
//! rollups.
//!
//! Same logic as `/v1/metrics:batch` — both write into the
//! `runtime_metrics_raw` partitioned table via
//! `MetricsStore::ingest_batch`. Distinct routes preserved for
//! legacy SDK compatibility (some SDKs hardcode one path or
//! the other).

pub use super::metrics::handle;
