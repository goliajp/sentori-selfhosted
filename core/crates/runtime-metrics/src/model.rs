//! Typed domain models.

use std::fmt;

use sentori_workspace_identity::ProjectId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;
use time::{Date, OffsetDateTime};

// ── input ────────────────────────────────────────────────────

/// One time-series point handed to
/// [`crate::MetricsStore::ingest_batch`].
///
/// The denormalised `release` / `environment` / `device_class`
/// columns can be set directly OR via the builder helpers
/// (`with_release`, etc.); the same values also live as JSONB
/// keys in `tags` so dashboard ad-hoc filters can read either
/// path.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricPoint {
    /// Owning project.
    pub project_id: ProjectId,
    /// Metric name (`app.startup_ms`, `app.frame_ms`, etc.).
    pub name: String,
    /// SDK-supplied timestamp.
    #[serde(with = "time::serde::rfc3339")]
    pub ts: OffsetDateTime,
    /// Sample value.
    pub value: f64,
    /// Free-form key→value tags. `release` / `environment` /
    /// `device_class` are also extracted into typed columns.
    #[serde(default)]
    pub tags: serde_json::Map<String, Value>,
    /// Denormalised release tag.
    #[serde(default)]
    pub release: Option<String>,
    /// Denormalised environment tag.
    #[serde(default)]
    pub environment: Option<String>,
    /// Denormalised device-class tag (`phone` / `tablet` /
    /// `web` / `tv`).
    #[serde(default)]
    pub device_class: Option<String>,
}

impl MetricPoint {
    /// Build a minimal point.
    #[must_use]
    pub fn new(
        project_id: ProjectId,
        name: impl Into<String>,
        ts: OffsetDateTime,
        value: f64,
    ) -> Self {
        Self {
            project_id,
            name: name.into(),
            ts,
            value,
            tags: serde_json::Map::new(),
            release: None,
            environment: None,
            device_class: None,
        }
    }

    /// Set the denormalised release.
    #[must_use]
    pub fn with_release(mut self, v: impl Into<String>) -> Self {
        self.release = Some(v.into());
        self
    }

    /// Set the denormalised environment.
    #[must_use]
    pub fn with_environment(mut self, v: impl Into<String>) -> Self {
        self.environment = Some(v.into());
        self
    }

    /// Set the denormalised device class.
    #[must_use]
    pub fn with_device_class(mut self, v: impl Into<String>) -> Self {
        self.device_class = Some(v.into());
        self
    }

    /// Add a free-form tag (also serialised into `tags` JSONB).
    #[must_use]
    pub fn with_tag(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.tags.insert(key.into(), value.into());
        self
    }

    /// Compute the canonical-JSON `tags_hash` used as part of
    /// the PK.
    ///
    /// Canonical form = serde_json sorted-keys serialisation
    /// of `tags` (BTreeMap-backed); SHA-256 truncated to 8
    /// bytes (signed i64).
    ///
    /// Stable across calls — two points with the same tag map
    /// (regardless of insertion order) produce the same hash.
    #[must_use]
    pub fn tags_hash(&self) -> i64 {
        canonical_tags_hash(&self.tags)
    }
}

/// Public stable hash used by the store + tests. Sorted-key
/// canonical-JSON SHA-256, low-8-bytes-as-i64.
#[must_use]
pub fn canonical_tags_hash(tags: &serde_json::Map<String, Value>) -> i64 {
    // BTreeMap-backed sorted serialisation. serde_json's Map
    // preserves insertion order — to canonicalise we explicitly
    // sort the keys + emit ordered pairs.
    let mut sorted: Vec<(&String, &Value)> = tags.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(b.0));
    let canonical = serde_json::to_vec(&sorted).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(&canonical);
    let digest = hasher.finalize();
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&digest[..8]);
    i64::from_le_bytes(buf)
}

// ── rollup row ───────────────────────────────────────────────

/// Which rollup tier a row belongs to. Caller picks which
/// table to read via [`crate::MetricsStore::query`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RollupTier {
    /// 1-minute buckets (60 rows per hour).
    Minute,
    /// 1-hour buckets (24 rows per day).
    Hour,
    /// 1-day buckets (30 rows per month).
    Day,
}

impl RollupTier {
    /// SQL table name for this tier.
    #[must_use]
    pub const fn table_name(self) -> &'static str {
        match self {
            Self::Minute => "runtime_metrics_1m",
            Self::Hour => "runtime_metrics_1h",
            Self::Day => "runtime_metrics_1d",
        }
    }
}

/// One row in any of `runtime_metrics_1m / _1h / _1d`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RollupRow {
    /// Bucket start (truncated to the tier's grain).
    pub bucket_ts: OffsetDateTime,
    /// Owning project.
    pub project_id: ProjectId,
    /// Metric name.
    pub name: String,
    /// Release dim.
    pub release: String,
    /// Environment dim.
    pub environment: String,
    /// Device class dim.
    pub device_class: String,
    /// Sample count in the bucket.
    pub count: i64,
    /// Sum of samples.
    pub sum: f64,
    /// avg = sum / count.
    pub avg: f64,
    /// 50th percentile.
    pub p50: f64,
    /// 95th percentile.
    pub p95: f64,
    /// 99th percentile.
    pub p99: f64,
}

// ── DropReason + DroppedRow ─────────────────────────────────

/// Stable reason tag for `runtime_metrics_dropped.reason`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DropReason {
    /// Caller-side rate limit said no.
    RateLimit,
    /// Input failed structural validation.
    Malformed,
    /// Value was NaN / ±Inf / outside SDK contract.
    InvalidValue,
    /// Backpressure path — caller's ingest buffer was full.
    OverCapacity,
}

impl DropReason {
    /// All four variants.
    pub const ALL: [Self; 4] = [
        Self::RateLimit,
        Self::Malformed,
        Self::InvalidValue,
        Self::OverCapacity,
    ];

    /// SQL wire form.
    #[must_use]
    pub const fn as_db_str(self) -> &'static str {
        match self {
            Self::RateLimit => "rate_limit",
            Self::Malformed => "malformed",
            Self::InvalidValue => "invalid_value",
            Self::OverCapacity => "over_capacity",
        }
    }

    /// Parse from the SQL wire form.
    ///
    /// # Errors
    ///
    /// [`DropReasonParseError`] for unknown strings.
    pub fn from_db_str(s: &str) -> Result<Self, DropReasonParseError> {
        match s {
            "rate_limit" => Ok(Self::RateLimit),
            "malformed" => Ok(Self::Malformed),
            "invalid_value" => Ok(Self::InvalidValue),
            "over_capacity" => Ok(Self::OverCapacity),
            other => Err(DropReasonParseError(other.to_string())),
        }
    }
}

impl fmt::Display for DropReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_db_str())
    }
}

/// Error from [`DropReason::from_db_str`].
#[derive(Debug, Error, PartialEq, Eq)]
#[error("unrecognised drop reason: {0:?}")]
pub struct DropReasonParseError(pub String);

/// `runtime_metrics_dropped` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DroppedRow {
    /// Day bucket.
    pub day: Date,
    /// Owning project.
    pub project_id: ProjectId,
    /// Why the drops happened.
    pub reason: DropReason,
    /// Cumulative count for the day.
    pub count: i64,
}

// ── shared row mapper ───────────────────────────────────────

// Result preserves shape with sibling K crates' `row_to_*`
// helpers; today this function is infallible but the enum-
// parse-failure path would surface here if rollup rows ever
// grow a typed column.
#[allow(clippy::unnecessary_wraps)]
pub(crate) fn row_to_rollup(
    row: &sqlx::postgres::PgRow,
) -> Result<RollupRow, crate::RuntimeMetricsError> {
    use sqlx::Row as _;
    Ok(RollupRow {
        bucket_ts: row.get("bucket_ts"),
        project_id: ProjectId::from_uuid(row.get("project_id")),
        name: row.get("name"),
        release: row.get("release"),
        environment: row.get("environment"),
        device_class: row.get("device_class"),
        count: row.get("count"),
        sum: row.get("sum"),
        avg: row.get("avg"),
        p50: row.get("p50"),
        p95: row.get("p95"),
        p99: row.get("p99"),
    })
}
