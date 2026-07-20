//! Typed domain models for the span-store surface.

use std::fmt;

use sentori_workspace_identity::ProjectId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::Cursor;

// ── status enum ─────────────────────────────────────────────

/// Span / trace status. Worst-of semantics on trace rollup:
/// any `Error` child marks the trace `Error`; otherwise
/// `Cancelled` beats `Ok`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SpanStatus {
    /// Span finished normally.
    Ok,
    /// Span failed (caller mapped exception → status).
    Error,
    /// Span aborted before completion (timeout, user cancel,
    /// task drop).
    Cancelled,
}

impl SpanStatus {
    /// All three variants in canonical UI order.
    pub const ALL: [Self; 3] = [Self::Ok, Self::Error, Self::Cancelled];

    /// SQL wire form (matches `CHECK status IN (...)`).
    #[must_use]
    pub const fn as_db_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Error => "error",
            Self::Cancelled => "cancelled",
        }
    }

    /// Parse from the SQL wire form.
    ///
    /// # Errors
    ///
    /// [`SpanStatusParseError`] for any unknown string.
    pub fn from_db_str(s: &str) -> Result<Self, SpanStatusParseError> {
        match s {
            "ok" => Ok(Self::Ok),
            "error" => Ok(Self::Error),
            "cancelled" => Ok(Self::Cancelled),
            other => Err(SpanStatusParseError(other.to_string())),
        }
    }

    /// "Worst" of `self` and `other` for trace-rollup
    /// promotion: `Error` > `Cancelled` > `Ok`. Order-
    /// independent.
    #[must_use]
    pub const fn worst_of(self, other: Self) -> Self {
        match (self, other) {
            (Self::Error, _) | (_, Self::Error) => Self::Error,
            (Self::Cancelled, _) | (_, Self::Cancelled) => Self::Cancelled,
            _ => Self::Ok,
        }
    }
}

impl fmt::Display for SpanStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_db_str())
    }
}

/// Error from [`SpanStatus::from_db_str`].
#[derive(Debug, Error, PartialEq, Eq)]
#[error("unrecognised span status: {0:?}")]
pub struct SpanStatusParseError(pub String);

// ── input ───────────────────────────────────────────────────

/// What [`crate::SpanStore::ingest_span`] takes.
///
/// `id` is server-minted if caller passes `Uuid::nil()` (the
/// SDK usually generates its own and posts it). `received_at`
/// is server-side wall-clock at ingest — caller should NOT
/// set it; we overwrite. `tags` / `data` accept SDK-side
/// JSON freely.
///
/// Set `parent_span_id = None` for the root span; the
/// presence/absence of `parent_span_id` is also how the
/// trace UPSERT decides whether to set `traces.root_op` /
/// `root_name` / `duration_ms`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpanInput {
    /// SDK-supplied span id (UUIDv7-ish). Server-minted if nil.
    pub id: Uuid,
    /// Trace this span joins.
    pub trace_id: Uuid,
    /// Parent span id. `None` for the trace's root span.
    #[serde(default)]
    pub parent_span_id: Option<Uuid>,
    /// SDK-supplied start time.
    #[serde(with = "time::serde::rfc3339")]
    pub started_at: OffsetDateTime,
    /// Wall-clock duration in milliseconds. Must be ≥ 0.
    pub duration_ms: i32,
    /// Operation tag (`http.client`, `db.query`, etc.). 1..=128
    /// chars.
    pub op: String,
    /// Human-readable name. 1..=512 chars.
    pub name: String,
    /// Status.
    pub status: SpanStatus,
    /// Arbitrary tags (`{ "http.method": "GET", … }`).
    #[serde(default)]
    pub tags: Value,
    /// Optional unstructured payload (request/response
    /// snippets, exception messages, etc.).
    #[serde(default)]
    pub data: Option<Value>,
    /// W3C `traceparent` header value the SDK propagated. Stored
    /// for cross-system trace stitching.
    #[serde(default)]
    pub traceparent: Option<String>,
}

// ── row reads ───────────────────────────────────────────────

/// `spans` row.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Span {
    /// Primary key (with `received_at`).
    pub id: Uuid,
    /// Owning project.
    pub project_id: ProjectId,
    /// Trace this span joined.
    pub trace_id: Uuid,
    /// Parent span (None for root).
    pub parent_span_id: Option<Uuid>,
    /// Server-side ingest timestamp (partition key).
    pub received_at: OffsetDateTime,
    /// SDK-supplied start.
    pub started_at: OffsetDateTime,
    /// Duration in ms.
    pub duration_ms: i32,
    /// Operation.
    pub op: String,
    /// Human-readable name.
    pub name: String,
    /// Status.
    pub status: SpanStatus,
    /// JSON tags.
    pub tags: Value,
    /// JSON optional unstructured data.
    pub data: Option<Value>,
    /// W3C traceparent.
    pub traceparent: Option<String>,
}

/// `traces` rollup row.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trace {
    /// Primary key.
    pub trace_id: Uuid,
    /// Owning project.
    pub project_id: ProjectId,
    /// Root span's `op`, or None until the root span lands.
    pub root_op: Option<String>,
    /// Root span's `name`.
    pub root_name: Option<String>,
    /// First child span received.
    pub first_seen: OffsetDateTime,
    /// Most recent child span received.
    pub last_seen: OffsetDateTime,
    /// How many spans have landed under this trace.
    pub span_count: i32,
    /// Worst-of status across all children.
    pub status: SpanStatus,
    /// Root span's `duration_ms`, or 0 until the root lands.
    pub duration_ms: i32,
}

impl Trace {
    /// True if the root span has not yet landed. Operators see
    /// these in the dashboard as "incomplete trace".
    #[must_use]
    pub const fn is_orphan(&self) -> bool {
        self.root_op.is_none()
    }
}

/// Trace + full ordered span list. Returned by
/// [`crate::SpanStore::trace_detail`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceDetail {
    /// Trace rollup row.
    pub trace: Trace,
    /// All spans of this trace, ordered by `started_at`
    /// ascending then by `id` ascending (deterministic
    /// waterfall ordering).
    pub spans: Vec<Span>,
}

/// Filters for [`crate::SpanStore::list_traces`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListTraceFilter {
    /// Restrict by trace status.
    #[serde(default)]
    pub status: Option<SpanStatus>,
    /// Restrict by root_op equality.
    #[serde(default)]
    pub root_op: Option<String>,
    /// Restrict to traces whose `last_seen` ≥ this value.
    #[serde(default, with = "time::serde::rfc3339::option")]
    pub last_seen_after: Option<OffsetDateTime>,
    /// Restrict to traces with `duration_ms ≥` this value (in
    /// ms). Useful for "show slow traces only".
    #[serde(default)]
    pub min_duration_ms: Option<i32>,
}

/// A page of traces + the cursor to fetch the next page.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedTraces {
    /// The traces on this page (already sorted desc by
    /// `last_seen`, tie-break by `trace_id`).
    pub items: Vec<Trace>,
    /// Cursor that asks for the next page, or `None` when
    /// the page is the last.
    pub next: Option<Cursor>,
}

// ── helpers shared with store.rs ────────────────────────────

pub(crate) fn row_to_span(row: &sqlx::postgres::PgRow) -> Result<Span, crate::SpanStoreError> {
    use sqlx::Row as _;
    let status_str: &str = row.get("status");
    Ok(Span {
        id: row.get("id"),
        project_id: ProjectId::from_uuid(row.get("project_id")),
        trace_id: row.get("trace_id"),
        parent_span_id: row.get("parent_span_id"),
        received_at: row.get("received_at"),
        started_at: row.get("started_at"),
        duration_ms: row.get("duration_ms"),
        op: row.get("op"),
        name: row.get("name"),
        status: SpanStatus::from_db_str(status_str)?,
        tags: row.get("tags"),
        data: row.get("data"),
        traceparent: row.get("traceparent"),
    })
}

pub(crate) fn row_to_trace(row: &sqlx::postgres::PgRow) -> Result<Trace, crate::SpanStoreError> {
    use sqlx::Row as _;
    let status_str: &str = row.get("status");
    Ok(Trace {
        trace_id: row.get("trace_id"),
        project_id: ProjectId::from_uuid(row.get("project_id")),
        root_op: row.get("root_op"),
        root_name: row.get("root_name"),
        first_seen: row.get("first_seen"),
        last_seen: row.get("last_seen"),
        span_count: row.get("span_count"),
        status: SpanStatus::from_db_str(status_str)?,
        duration_ms: row.get("duration_ms"),
    })
}
