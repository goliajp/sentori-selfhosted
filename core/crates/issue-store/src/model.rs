//! Typed domain models for the issue-store surface.

use sentori_event_pipeline::{EventKind, IssueStatus, Platform};
use sentori_workspace_identity::{ProjectId, UserId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::cursor::Cursor;

// ── filters / patches ────────────────────────────────────────

/// Filters applied to [`crate::IssueStore::list`].
///
/// All fields are optional; an empty `ListFilter` returns
/// every active issue for the project (sorted by `last_seen`
/// desc).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListFilter {
    /// Restrict by status. `None` includes `Active` only
    /// (operator's typical default); `Some(s)` includes only
    /// `s`; `any` shortcut via `Some(IssueStatus::Active)` …
    /// — explicit: pass the variant you want.
    #[serde(default)]
    pub status: Option<IssueStatus>,
    /// Restrict by environment (`production`, `staging`, …).
    /// Matches against the issue's `last_environment`.
    #[serde(default)]
    pub environment: Option<String>,
    /// Restrict by `last_release` equality.
    #[serde(default)]
    pub release: Option<String>,
    /// Restrict by `error_type` equality. Useful for "show me
    /// every NullPointerException".
    #[serde(default)]
    pub error_type: Option<String>,
    /// Substring search (ILIKE) over `error_type` +
    /// `message_sample`.
    #[serde(default)]
    pub search: Option<String>,
    /// Priority filter — any-of match if non-empty.
    #[serde(default)]
    pub priorities: Vec<String>,
    /// Labels filter — any-of match (issue with ≥1 of these
    /// labels passes).
    #[serde(default)]
    pub labels: Vec<String>,
    /// Restrict to issues whose `last_seen` is at or after.
    #[serde(default, with = "time::serde::rfc3339::option")]
    pub last_seen_after: Option<OffsetDateTime>,
    /// Restrict to issues assigned to a specific user (or
    /// pass `None` for "any assignment state").
    #[serde(default)]
    pub assignee_user_id: Option<UserId>,
}

/// Triage patch applied to one or more issues. Every field
/// is optional — only the non-`None` fields are written.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssuePatch {
    /// New status (`Active` / `Resolved` / `Regressed` /
    /// `Ignored`). Setting `Resolved` also stamps
    /// `resolved_at` from the patch's `now` parameter.
    #[serde(default)]
    pub status: Option<IssueStatus>,
    /// New assignee. `Some(Some(uid))` to assign; `Some(None)`
    /// to clear; `None` to leave unchanged.
    #[serde(default)]
    pub assignee_user_id: Option<Option<UserId>>,
    /// New priority. Must be `p0`/`p1`/`p2`/`p3`.
    #[serde(default)]
    pub priority: Option<String>,
    /// Replace the labels set (not additive — caller computes
    /// the new set client-side).
    #[serde(default)]
    pub labels: Option<Vec<String>>,
    /// Operator-supplied release the resolution is recorded
    /// against (the build the operator believes contains the
    /// fix). Only stored on `Resolved` transitions; ignored
    /// otherwise.
    #[serde(default)]
    pub resolved_in_release: Option<String>,
}

// ── return shapes ────────────────────────────────────────────

/// One row in a [`PaginatedIssues`] result.
///
/// `event_count`, `last_seen`, `first_seen` etc come from
/// the K4 [`sentori_event_pipeline::Issue`] columns; the K5
/// additions (`assignee_user_id`, `priority`, `labels`,
/// `resolved_in_release`) come from migration 0004.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueSummary {
    /// Issue id.
    pub id: Uuid,
    /// Owning project.
    pub project_id: ProjectId,
    /// Hash key.
    pub fingerprint: String,
    /// Exception class / event-kind synthetic for `Message`.
    pub error_type: String,
    /// First-event message body (display only).
    pub message_sample: String,
    /// Event kind of the first event.
    pub kind: EventKind,
    /// Lifecycle status.
    pub status: IssueStatus,
    /// Triage priority (`p0`/`p1`/`p2`/`p3`).
    pub priority: String,
    /// Operator-typed labels.
    pub labels: Vec<String>,
    /// Operator assignment (or `None` if unassigned).
    pub assignee_user_id: Option<UserId>,
    /// First event timestamp.
    pub first_seen: OffsetDateTime,
    /// Last event timestamp (the cursor sort key).
    pub last_seen: OffsetDateTime,
    /// Cumulative event count for this issue.
    pub event_count: i64,
    /// Environment of the most recent event.
    pub last_environment: String,
    /// Release of the most recent event.
    pub last_release: String,
    /// Set when the issue regressed.
    pub regressed_at: Option<OffsetDateTime>,
    /// Release that triggered the regression.
    pub regressed_in_release: Option<String>,
    /// Set when the operator marked the issue resolved.
    pub resolved_at: Option<OffsetDateTime>,
    /// Release the resolution was filed against.
    pub resolved_in_release: Option<String>,
}

/// A page of issues + the cursor to fetch the next page.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedIssues {
    /// The issues on this page (already sorted desc by
    /// `last_seen`, tie-break by `id`).
    pub items: Vec<IssueSummary>,
    /// Cursor that asks for the next page, or `None` when
    /// `items.len() < cursor.limit` (i.e. the result was
    /// short — no more rows to walk).
    pub next: Option<Cursor>,
}

/// A page of events under one issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedEvents {
    /// Events on this page (desc by `timestamp`, tie-break
    /// by `id`).
    pub items: Vec<EventCursorRow>,
    /// Cursor for the next page.
    pub next: Option<Cursor>,
}

/// Re-export of K4's stored-event shape for use in
/// `PaginatedEvents` — kept as `pub use` so consumers don't
/// need to import K4 directly.
pub use sentori_event_pipeline::StoredEvent as EventCursorRow;

/// Re-export aliases so the lib.rs surface reads clearly.
pub use crate::cursor::Cursor as IssueCursor;
/// Re-export — same `Cursor` type drives both list paths.
pub use crate::cursor::Cursor as EventCursor;

/// Full issue detail, returned by [`crate::IssueStore::detail`].
///
/// Adds [`AffectedUsers`] (a privacy-aware count of distinct
/// `payload.user.id` values across the issue's events) on
/// top of [`IssueSummary`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueDetail {
    /// The summary fields (id / status / priority / etc).
    pub summary: IssueSummary,
    /// Distinct-user count + sample window size.
    pub affected_users: AffectedUsers,
}

/// Result of the affected-users subquery.
///
/// `count` is the distinct identifier count over the latest
/// `sampled_events` events under the issue. Caps the scan to
/// keep the query under a few ms on big issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AffectedUsers {
    /// Distinct non-null `payload.user.id` strings observed
    /// in the sample.
    pub count: i64,
    /// Number of events the count was computed over (≤ the
    /// store's sample cap).
    pub sampled_events: i64,
    /// True when the sample window had to cap (issue has more
    /// events than the sample cap). UI can then label
    /// "≥ N affected" instead of "N affected exactly".
    pub truncated: bool,
}

/// One row in [`crate::IssueStore::related`]. Same shape as
/// [`IssueSummary`] but with the relation field telling the
/// UI WHY it was returned.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelatedIssue {
    /// The related issue's summary.
    pub summary: IssueSummary,
    /// Why the matcher classified this as related.
    pub relation: RelationReason,
}

/// Why a related issue was matched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationReason {
    /// Same `error_type`, different `last_release` (the
    /// "did the bug come back?" cross-release lookup).
    SameTypeDifferentRelease,
    /// Same `kind`, same `error_type`, same `message_sample`,
    /// different `fingerprint` — typically same root cause
    /// across slightly different stacks.
    SameSignatureDifferentFingerprint,
}

/// Result of [`crate::IssueStore::patch`] /
/// [`crate::IssueStore::bulk_patch`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchOutcome {
    /// Number of rows the UPDATE affected.
    pub updated: u64,
    /// True if any of the patched issues transitioned to
    /// `Resolved` (consumer notifier can fire "issue resolved"
    /// emails off this).
    pub any_resolved: bool,
    /// True if any reverted from `Resolved` back to `Active`
    /// (operator un-resolved). Distinct from K4's automatic
    /// regression detection — that fires on ingest; this is
    /// an explicit operator action.
    pub any_reopened: bool,
}

/// Result of [`crate::IssueStore::merge`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeOutcome {
    /// Destination issue id (caller-supplied).
    pub dst: Uuid,
    /// Source issue id (deleted).
    pub src: Uuid,
    /// How many event rows were re-pointed from src to dst.
    pub events_moved: u64,
}

/// Convenience — a re-export of K4's [`sentori_event_pipeline::Issue`]
/// for callers wanting the raw row. Most paths should prefer
/// [`IssueSummary`] (which adds the K5 triage columns).
pub use sentori_event_pipeline::Issue as RawIssue;

/// A re-export of [`serde_json::Value`] used by the
/// `affected_users` query; helps consumer crates avoid
/// importing serde_json directly.
pub use serde_json::Value as PayloadValue;

// silence "unused" — module-public re-exports above are
// indirectly used by consumer crates.
#[allow(dead_code)]
const _UNUSED_RAW: Option<RawIssue> = None;
#[allow(dead_code)]
const _UNUSED_VAL: Option<PayloadValue> = None;

// ── helpers used by store.rs ────────────────────────────────

/// Mapping helper used by store.rs to read issues+triage rows.
/// Pure SQL-row → typed-summary; doesn't touch the pool.
pub(crate) fn row_to_summary(
    row: &sqlx::postgres::PgRow,
) -> Result<IssueSummary, crate::IssueStoreError> {
    use sqlx::Row as _;
    let kind_str: &str = row.get("kind");
    let status_str: &str = row.get("status");
    Ok(IssueSummary {
        id: row.get("id"),
        project_id: ProjectId::from_uuid(row.get("project_id")),
        fingerprint: row.get("fingerprint"),
        error_type: row.get("error_type"),
        message_sample: row.get("message_sample"),
        kind: EventKind::from_db_str(kind_str)?,
        status: IssueStatus::from_db_str(status_str)?,
        priority: row.get("priority"),
        labels: row.get::<Vec<String>, _>("labels"),
        assignee_user_id: row
            .get::<Option<Uuid>, _>("assignee_user_id")
            .map(UserId::from_uuid),
        first_seen: row.get("first_seen"),
        last_seen: row.get("last_seen"),
        event_count: row.get("event_count"),
        last_environment: row.get("last_environment"),
        last_release: row.get("last_release"),
        regressed_at: row.get("regressed_at"),
        regressed_in_release: row.get("regressed_in_release"),
        resolved_at: row.get("resolved_at"),
        resolved_in_release: row.get("resolved_in_release"),
    })
}

/// Mapping helper for the events table — shared with K4's
/// stored-event shape. We re-derive here so a future K5-only
/// projection (smaller payload, hot path) can swap in without
/// touching call sites.
pub(crate) fn row_to_event(
    row: &sqlx::postgres::PgRow,
) -> Result<sentori_event_pipeline::StoredEvent, crate::IssueStoreError> {
    use sqlx::Row as _;
    let kind_str: &str = row.get("kind");
    let platform_str: &str = row.get("platform");
    Ok(sentori_event_pipeline::StoredEvent {
        id: row.get("id"),
        project_id: ProjectId::from_uuid(row.get("project_id")),
        issue_id: row.get("issue_id"),
        timestamp: row.get("timestamp"),
        kind: EventKind::from_db_str(kind_str)?,
        platform: Platform::from_db_str(platform_str).map_err(|_| {
            crate::IssueStoreError::Db(sqlx::Error::Protocol("invalid platform".into()))
        })?,
        release: row.get("release"),
        environment: row.get("environment"),
        payload: row.get::<Value, _>("payload"),
        received_at: row.get("received_at"),
    })
}
