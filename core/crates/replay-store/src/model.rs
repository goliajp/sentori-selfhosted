//! Typed domain models for the replay-store surface.

use sentori_workspace_identity::ProjectId;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::Cursor;

/// `replay_sessions` row.
///
/// The `blob_hash` field is the SHA-256 hex of the gzipped
/// scrubbed NDJSON; the actual bytes live in K3 attachment-
/// store. Use [`crate::ReplayStore::fetch`] to load.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplaySession {
    /// Primary key.
    pub id: Uuid,
    /// Owning project.
    pub project_id: ProjectId,
    /// The event this replay attaches to.
    pub event_id: Uuid,
    /// Hex SHA-256 of the gzipped scrubbed bytes in K3.
    pub blob_hash: String,
    /// Wall-clock start of the replay window (SDK-supplied).
    pub started_at: OffsetDateTime,
    /// Wall-clock end of the replay window.
    pub ended_at: OffsetDateTime,
    /// NDJSON line count (keyframes + deltas).
    pub frame_count: i32,
    /// How many text-node values were redacted by the scrubber.
    /// Zero means a clean session; > 0 flags it in the
    /// dashboard.
    pub scrubbed_count: i32,
    /// Post-scrub post-gzip byte count, for dashboard
    /// storage-estimation badges.
    pub byte_count: i32,
    /// Insert timestamp.
    pub created_at: OffsetDateTime,
}

impl ReplaySession {
    /// True if the scrubber redacted anything in this session.
    #[must_use]
    pub const fn had_pii(&self) -> bool {
        self.scrubbed_count > 0
    }

    /// Duration of the captured window.
    #[must_use]
    pub fn window_duration(&self) -> time::Duration {
        self.ended_at - self.started_at
    }
}

/// A page of replay sessions + the cursor for the next.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedReplays {
    /// Sessions on this page (sorted desc by `created_at`,
    /// tie-break by `id`).
    pub items: Vec<ReplaySession>,
    /// Cursor for the next page, or `None` when the page is
    /// the last.
    pub next: Option<Cursor>,
}

// ── helpers shared with store.rs ────────────────────────────

// Result return preserves shape with sibling K crates'
// `row_to_*` helpers; today it never errors but the enum-
// parse-failure path would surface here in a future revision.
#[allow(clippy::unnecessary_wraps)]
pub(crate) fn row_to_session(
    row: &sqlx::postgres::PgRow,
) -> Result<ReplaySession, crate::ReplayStoreError> {
    use sqlx::Row as _;
    Ok(ReplaySession {
        id: row.get("id"),
        project_id: ProjectId::from_uuid(row.get("project_id")),
        event_id: row.get("event_id"),
        blob_hash: row.get("blob_hash"),
        started_at: row.get("started_at"),
        ended_at: row.get("ended_at"),
        frame_count: row.get("frame_count"),
        scrubbed_count: row.get("scrubbed_count"),
        byte_count: row.get("byte_count"),
        created_at: row.get("created_at"),
    })
}
