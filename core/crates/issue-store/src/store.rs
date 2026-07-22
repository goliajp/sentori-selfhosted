//! [`IssueStore`] — the public handle wrapping the K4-owned
//! `issues` + `events` tables with operator read + mutate.

use sentori_event_pipeline::IssueStatus;
use sentori_workspace_identity::{ProjectId, UserId, WorkspaceId};
use sqlx::{PgPool, Row};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::cursor::Cursor;
use crate::error::IssueStoreError;
use crate::model::{
    AffectedUsers, IssueDetail, IssuePatch, IssueSummary, ListFilter, MergeOutcome,
    PaginatedEvents, PaginatedIssues, PatchOutcome, RelatedIssue, RelationReason,
};

/// Allowed `priority` strings — kept here, not in `model.rs`,
/// because validation is a store-layer concern (the SQL CHECK
/// constraint is the authoritative gate; this is the early
/// reject so the dashboard gets a typed error instead of a DB
/// error).
const ALLOWED_PRIORITIES: &[&str] = &["p0", "p1", "p2", "p3"];

/// Number of events the `affected_users` subquery samples
/// per issue. Caps the JSONB scan cost — beyond 5k events
/// the count is reported as `truncated`.
const AFFECTED_USERS_SAMPLE_CAP: i64 = 5_000;

/// Public handle.
#[derive(Debug, Clone)]
pub struct IssueStore {
    pool: PgPool,
}

impl IssueStore {
    /// Construct over a connection pool.
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Borrow the pool — convenience for tests + ad-hoc
    /// queries in the consumer crate.
    #[must_use]
    pub const fn pool(&self) -> &PgPool {
        &self.pool
    }

    // ── read ────────────────────────────────────────────────

    /// List issues for a project, filtered + cursor-paginated.
    ///
    /// Sort order is `(last_seen DESC, id DESC)`. Tie-break by
    /// id keeps cursor walks deterministic when two issues
    /// share a `last_seen` (rare but possible — same UUIDv7
    /// minute can produce identical millisecond timestamps).
    ///
    /// # Errors
    ///
    /// [`IssueStoreError::Db`] / [`IssueStoreError::InvalidKindInDb`].
    pub async fn list(
        &self,
        project_id: ProjectId,
        filter: ListFilter,
        cursor: Cursor,
    ) -> Result<PaginatedIssues, IssueStoreError> {
        let priorities_arr = if filter.priorities.is_empty() {
            None
        } else {
            Some(filter.priorities)
        };
        let labels_arr = if filter.labels.is_empty() {
            None
        } else {
            Some(filter.labels)
        };
        let search_pattern = filter
            .search
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| format!("%{s}%"));

        // Fetch one extra row so we can tell "is there a next
        // page" without a second query. If we get back
        // `limit + 1`, trim and emit a cursor; otherwise the
        // page is the last.
        let fetch_limit = i64::from(cursor.limit) + 1;
        let (anchor_ts, anchor_id) = match cursor.anchor {
            Some((ts, id)) => (Some(ts), Some(id)),
            None => (None, None),
        };

        let rows = sqlx::query(
            r"
            SELECT id, project_id, fingerprint, error_type, message_sample, kind,
                   status, priority, labels, assignee_user_id,
                   first_seen, last_seen, event_count,
                   last_environment, last_release,
                   regressed_at, regressed_in_release,
                   resolved_at, resolved_in_release
            FROM issues
            WHERE project_id = $1
              AND ($2::text IS NULL OR status = $2)
              AND ($3::text IS NULL OR last_environment = $3)
              AND ($4::text IS NULL OR last_release = $4)
              AND ($5::text IS NULL OR error_type = $5)
              AND ($6::text IS NULL OR (
                    error_type ILIKE $6 OR message_sample ILIKE $6
                  ))
              AND ($7::text[] IS NULL OR priority = ANY($7))
              AND ($8::text[] IS NULL OR labels && $8)
              AND ($9::timestamptz IS NULL OR last_seen >= $9)
              AND ($10::uuid IS NULL OR assignee_user_id = $10)
              AND (
                    $11::timestamptz IS NULL
                    OR (last_seen, id) < ($11::timestamptz, $12::uuid)
                  )
            ORDER BY last_seen DESC, id DESC
            LIMIT $13
            ",
        )
        .bind(project_id.into_uuid())
        .bind(filter.status.map(IssueStatus::as_db_str))
        .bind(filter.environment.as_deref())
        .bind(filter.release.as_deref())
        .bind(filter.error_type.as_deref())
        .bind(search_pattern.as_deref())
        .bind(priorities_arr.as_deref())
        .bind(labels_arr.as_deref())
        .bind(filter.last_seen_after)
        .bind(filter.assignee_user_id.map(UserId::into_uuid))
        .bind(anchor_ts)
        .bind(anchor_id)
        .bind(fetch_limit)
        .fetch_all(&self.pool)
        .await?;

        let mut items: Vec<IssueSummary> = rows
            .iter()
            .map(crate::model::row_to_summary)
            .collect::<Result<_, _>>()?;

        let next = if items.len() as i64 > i64::from(cursor.limit) {
            // We over-fetched by 1 to detect a next page; drop
            // the lookahead row. The cursor anchors on the
            // LAST kept item — next page's `(last_seen, id) <
            // anchor` strictly skips past it.
            let _trimmed = items.pop();
            let last = items.last().expect("len > 0 after pop");
            Some(Cursor::next(last.last_seen, last.id, cursor.limit))
        } else {
            None
        };

        Ok(PaginatedIssues { items, next })
    }

    /// Full detail of one issue + an affected-users count.
    ///
    /// # Errors
    ///
    /// [`IssueStoreError::IssueNotFound`] if no such issue;
    /// [`IssueStoreError::Db`] on database failure.
    pub async fn detail(&self, issue_id: Uuid) -> Result<IssueDetail, IssueStoreError> {
        let row = sqlx::query(
            r"
            SELECT id, project_id, fingerprint, error_type, message_sample, kind,
                   status, priority, labels, assignee_user_id,
                   first_seen, last_seen, event_count,
                   last_environment, last_release,
                   regressed_at, regressed_in_release,
                   resolved_at, resolved_in_release
            FROM issues
            WHERE id = $1
            ",
        )
        .bind(issue_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(IssueStoreError::IssueNotFound(issue_id))?;
        let summary = crate::model::row_to_summary(&row)?;
        let affected_users = self.affected_users(issue_id).await?;
        Ok(IssueDetail {
            summary,
            affected_users,
        })
    }

    /// Distinct-user count for an issue, sampled over the
    /// latest 5 000 events.
    ///
    /// Reads `payload -> 'user' ->> 'id'` (the K4 Event's
    /// optional `payload.user.id` field). Sample is the most
    /// recent N events ordered by `timestamp DESC`.
    ///
    /// # Errors
    ///
    /// [`IssueStoreError::Db`] on database failure.
    pub async fn affected_users(&self, issue_id: Uuid) -> Result<AffectedUsers, IssueStoreError> {
        let row = sqlx::query(
            r"
            WITH sample AS (
                SELECT payload -> 'user' ->> 'id' AS user_id
                FROM events
                WHERE issue_id = $1
                ORDER BY timestamp DESC
                LIMIT $2
            ),
            stats AS (
                SELECT
                    COUNT(*)::bigint                                       AS sampled,
                    COUNT(DISTINCT user_id) FILTER (WHERE user_id IS NOT NULL)::bigint
                                                                          AS distinct_users
                FROM sample
            ),
            total AS (
                SELECT COUNT(*)::bigint AS total FROM events WHERE issue_id = $1
            )
            SELECT
                stats.distinct_users,
                stats.sampled,
                (total.total > $2)::bool AS truncated
            FROM stats, total
            ",
        )
        .bind(issue_id)
        .bind(AFFECTED_USERS_SAMPLE_CAP)
        .fetch_one(&self.pool)
        .await?;

        Ok(AffectedUsers {
            count: row.get::<i64, _>("distinct_users"),
            sampled_events: row.get::<i64, _>("sampled"),
            truncated: row.get::<bool, _>("truncated"),
        })
    }

    /// Cross-release "did the bug come back?" panel.
    ///
    /// Returns up to `limit` related issues by two heuristics:
    ///
    /// 1. **Same error_type, different last_release** — the
    ///    canonical use case (operator marked X@1.0 resolved,
    ///    same error appears in X@2.0).
    /// 2. **Same signature (kind + error_type + message_sample),
    ///    different fingerprint** — root-cause-likely peers
    ///    where the stack frame split the fingerprint.
    ///
    /// # Errors
    ///
    /// [`IssueStoreError::IssueNotFound`] if `issue_id`
    /// doesn't exist; [`IssueStoreError::Db`] on failure.
    pub async fn related(
        &self,
        issue_id: Uuid,
        limit: u32,
    ) -> Result<Vec<RelatedIssue>, IssueStoreError> {
        let limit_clamped = i64::from(limit.clamp(1, 100));
        let anchor = sqlx::query(
            r"
            SELECT project_id, kind, error_type, message_sample, last_release
            FROM issues WHERE id = $1
            ",
        )
        .bind(issue_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(IssueStoreError::IssueNotFound(issue_id))?;

        let project_id: Uuid = anchor.get("project_id");
        let kind: String = anchor.get("kind");
        let error_type: String = anchor.get("error_type");
        let message_sample: String = anchor.get("message_sample");
        let last_release: String = anchor.get("last_release");

        // Two passes — UNION ALL would conflate the
        // `relation` reason; clean to run two queries +
        // dedupe in Rust.
        let cross_release_rows = sqlx::query(
            r"
            SELECT id, project_id, fingerprint, error_type, message_sample, kind,
                   status, priority, labels, assignee_user_id,
                   first_seen, last_seen, event_count,
                   last_environment, last_release,
                   regressed_at, regressed_in_release,
                   resolved_at, resolved_in_release
            FROM issues
            WHERE project_id = $1
              AND error_type = $2
              AND last_release <> $3
              AND id <> $4
            ORDER BY last_seen DESC
            LIMIT $5
            ",
        )
        .bind(project_id)
        .bind(&error_type)
        .bind(&last_release)
        .bind(issue_id)
        .bind(limit_clamped)
        .fetch_all(&self.pool)
        .await?;

        let same_signature_rows = sqlx::query(
            r"
            SELECT id, project_id, fingerprint, error_type, message_sample, kind,
                   status, priority, labels, assignee_user_id,
                   first_seen, last_seen, event_count,
                   last_environment, last_release,
                   regressed_at, regressed_in_release,
                   resolved_at, resolved_in_release
            FROM issues
            WHERE project_id = $1
              AND kind = $2
              AND error_type = $3
              AND message_sample = $4
              AND id <> $5
            ORDER BY last_seen DESC
            LIMIT $6
            ",
        )
        .bind(project_id)
        .bind(&kind)
        .bind(&error_type)
        .bind(&message_sample)
        .bind(issue_id)
        .bind(limit_clamped)
        .fetch_all(&self.pool)
        .await?;

        let mut out: Vec<RelatedIssue> =
            Vec::with_capacity(cross_release_rows.len() + same_signature_rows.len());
        let mut seen_ids = std::collections::HashSet::new();
        for r in &cross_release_rows {
            let s = crate::model::row_to_summary(r)?;
            if seen_ids.insert(s.id) {
                out.push(RelatedIssue {
                    summary: s,
                    relation: RelationReason::SameTypeDifferentRelease,
                });
            }
        }
        for r in &same_signature_rows {
            let s = crate::model::row_to_summary(r)?;
            if seen_ids.insert(s.id) {
                out.push(RelatedIssue {
                    summary: s,
                    relation: RelationReason::SameSignatureDifferentFingerprint,
                });
            }
        }
        out.truncate(limit_clamped as usize);
        Ok(out)
    }

    /// Distinct releases observed for this issue.
    ///
    /// Walks the `events` table for distinct `release` values
    /// — cheap on small issue volumes, indexed via
    /// `events_issue_timestamp_idx`.
    ///
    /// # Errors
    ///
    /// [`IssueStoreError::Db`] on database failure.
    pub async fn releases_for_issue(&self, issue_id: Uuid) -> Result<Vec<String>, IssueStoreError> {
        let rows = sqlx::query(
            r"
            SELECT DISTINCT release
            FROM events
            WHERE issue_id = $1
            ORDER BY release
            ",
        )
        .bind(issue_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| r.get::<String, _>("release"))
            .collect())
    }

    /// Cursor-paginated events for an issue.
    ///
    /// # Errors
    ///
    /// [`IssueStoreError::Db`] on database failure.
    pub async fn events_for_issue(
        &self,
        issue_id: Uuid,
        cursor: Cursor,
    ) -> Result<PaginatedEvents, IssueStoreError> {
        let fetch_limit = i64::from(cursor.limit) + 1;
        let (anchor_ts, anchor_id) = match cursor.anchor {
            Some((ts, id)) => (Some(ts), Some(id)),
            None => (None, None),
        };

        let rows = sqlx::query(
            r"
            SELECT id, project_id, issue_id, timestamp, kind, platform,
                   release, environment, payload, received_at
            FROM events
            WHERE issue_id = $1
              AND (
                    $2::timestamptz IS NULL
                    OR (timestamp, id) < ($2::timestamptz, $3::uuid)
                  )
            ORDER BY timestamp DESC, id DESC
            LIMIT $4
            ",
        )
        .bind(issue_id)
        .bind(anchor_ts)
        .bind(anchor_id)
        .bind(fetch_limit)
        .fetch_all(&self.pool)
        .await?;

        let mut items = rows
            .iter()
            .map(crate::model::row_to_event)
            .collect::<Result<Vec<_>, _>>()?;

        let next = if items.len() as i64 > i64::from(cursor.limit) {
            let _trimmed = items.pop();
            let last = items.last().expect("non-empty by guard");
            Some(Cursor::next(last.timestamp, last.id, cursor.limit))
        } else {
            None
        };

        Ok(PaginatedEvents { items, next })
    }

    // ── mutate ──────────────────────────────────────────────

    /// Apply a triage patch to one issue.
    ///
    /// # Errors
    ///
    /// - [`IssueStoreError::IssueNotFound`] if no row matches.
    /// - [`IssueStoreError::InvalidPriority`] for a non-canonical
    ///   priority string.
    /// - [`IssueStoreError::Db`] on database failure.
    pub async fn patch(
        &self,
        workspace_id: WorkspaceId,
        issue_id: Uuid,
        patch: IssuePatch,
        now: OffsetDateTime,
    ) -> Result<PatchOutcome, IssueStoreError> {
        // Treat single-row not-found as a 404 rather than a
        // silent zero — operator-facing UX is clearer. Bulk
        // path silently no-ops on missing ids by design; here
        // we surface.
        let out = self
            .bulk_patch(workspace_id, &[issue_id], patch, now)
            .await?;
        if out.updated == 0 {
            Err(IssueStoreError::IssueNotFound(issue_id))
        } else {
            Ok(out)
        }
    }

    /// Apply the same patch to multiple issues in one
    /// transaction. Missing ids are silent (no error per
    /// id); the `updated` count tells the caller how many
    /// matched.
    ///
    /// # Errors
    ///
    /// - [`IssueStoreError::InvalidPriority`] if `patch.priority`
    ///   is non-canonical.
    /// - [`IssueStoreError::Db`] on database failure.
    pub async fn bulk_patch(
        &self,
        workspace_id: WorkspaceId,
        ids: &[Uuid],
        patch: IssuePatch,
        now: OffsetDateTime,
    ) -> Result<PatchOutcome, IssueStoreError> {
        if let Some(p) = patch.priority.as_deref()
            && !ALLOWED_PRIORITIES.contains(&p)
        {
            return Err(IssueStoreError::InvalidPriority { got: p.to_string() });
        }
        if ids.is_empty() {
            return Ok(PatchOutcome::default());
        }

        // Detect pre-patch transitions for any_resolved /
        // any_reopened in the same tx so callers can fire
        // notifications without a separate read.
        let mut tx = self.pool.begin().await?;
        let pre: Vec<(Uuid, String)> = sqlx::query(
            "SELECT id, status FROM issues \
                 WHERE id = ANY($1) AND workspace_id = $2 FOR UPDATE",
        )
        .bind(ids)
        .bind(workspace_id.into_uuid())
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .map(|r| (r.get::<Uuid, _>("id"), r.get::<String, _>("status")))
        .collect();

        // Build the UPDATE — every field is COALESCE'd off
        // the patch so unset fields preserve the existing
        // value. Some fields are tri-state (assignee can be
        // set OR cleared OR left); we encode "clear" as a
        // sentinel column.
        let new_status = patch.status.map(IssueStatus::as_db_str);
        let (assignee_set, assignee_value): (bool, Option<Uuid>) = match patch.assignee_user_id {
            Some(Some(u)) => (true, Some(u.into_uuid())),
            Some(None) => (true, None),
            None => (false, None),
        };
        let result = sqlx::query(
            r"
            UPDATE issues SET
                status = COALESCE($2, status),
                priority = COALESCE($3, priority),
                labels = COALESCE($4, labels),
                assignee_user_id = CASE WHEN $5 THEN $6 ELSE assignee_user_id END,
                resolved_at = CASE
                    WHEN $2 = 'resolved' THEN $7
                    WHEN $2 IS NOT NULL AND $2 <> 'resolved' THEN NULL
                    ELSE resolved_at
                END,
                resolved_in_release = CASE
                    WHEN $2 = 'resolved' THEN COALESCE($8, resolved_in_release)
                    WHEN $2 IS NOT NULL AND $2 <> 'resolved' THEN NULL
                    ELSE resolved_in_release
                END
            WHERE id = ANY($1) AND workspace_id = $9
            ",
        )
        .bind(ids)
        .bind(new_status)
        .bind(patch.priority.as_deref())
        .bind(patch.labels.as_deref())
        .bind(assignee_set)
        .bind(assignee_value)
        .bind(now)
        .bind(patch.resolved_in_release.as_deref())
        .bind(workspace_id.into_uuid())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        let updated = result.rows_affected();

        let any_resolved =
            matches!(new_status, Some("resolved")) && pre.iter().any(|(_, s)| s != "resolved");
        // "Reopened" = transition FROM resolved/regressed to
        // active. Regressed back to active is an explicit
        // operator action (K4 handles automatic regression
        // detection on the ingest path).
        let any_reopened = matches!(new_status, Some("active"))
            && pre.iter().any(|(_, s)| s == "resolved" || s == "regressed");

        Ok(PatchOutcome {
            updated,
            any_resolved,
            any_reopened,
        })
    }

    /// Merge `src` into `dst`: move every event row from src
    /// to dst, bump dst's counts, then delete src.
    ///
    /// Both issues must belong to the same project; merging
    /// across project boundaries is refused.
    ///
    /// # Errors
    ///
    /// - [`IssueStoreError::MergeIntoSelf`] if `src == dst`.
    /// - [`IssueStoreError::IssueNotFound`] if either side is
    ///   missing.
    /// - [`IssueStoreError::MergeAcrossProjects`] for
    ///   different-project pairs.
    /// - [`IssueStoreError::Db`] on database failure.
    pub async fn merge(&self, src: Uuid, dst: Uuid) -> Result<MergeOutcome, IssueStoreError> {
        if src == dst {
            return Err(IssueStoreError::MergeIntoSelf);
        }

        let mut tx = self.pool.begin().await?;

        let pre: Vec<(Uuid, Uuid, OffsetDateTime, OffsetDateTime, i64)> = sqlx::query(
            "SELECT id, project_id, first_seen, last_seen, event_count
             FROM issues WHERE id = ANY($1) FOR UPDATE",
        )
        .bind(&[src, dst][..])
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .map(|r| {
            (
                r.get::<Uuid, _>("id"),
                r.get::<Uuid, _>("project_id"),
                r.get::<OffsetDateTime, _>("first_seen"),
                r.get::<OffsetDateTime, _>("last_seen"),
                r.get::<i64, _>("event_count"),
            )
        })
        .collect();

        let src_row = pre
            .iter()
            .find(|(id, _, _, _, _)| *id == src)
            .ok_or(IssueStoreError::IssueNotFound(src))?;
        let dst_row = pre
            .iter()
            .find(|(id, _, _, _, _)| *id == dst)
            .ok_or(IssueStoreError::IssueNotFound(dst))?;
        if src_row.1 != dst_row.1 {
            return Err(IssueStoreError::MergeAcrossProjects);
        }

        // Move events.
        let move_result = sqlx::query("UPDATE events SET issue_id = $1 WHERE issue_id = $2")
            .bind(dst)
            .bind(src)
            .execute(&mut *tx)
            .await?;
        let events_moved = move_result.rows_affected();

        // Bump dst's bookkeeping (event_count, last_seen,
        // first_seen).
        sqlx::query(
            r"
            UPDATE issues SET
                event_count = event_count + $1,
                first_seen  = LEAST(first_seen, $2),
                last_seen   = GREATEST(last_seen, $3)
            WHERE id = $4
            ",
        )
        .bind(src_row.4)
        .bind(src_row.2)
        .bind(src_row.3)
        .bind(dst)
        .execute(&mut *tx)
        .await?;

        // Delete the source row.
        sqlx::query("DELETE FROM issues WHERE id = $1")
            .bind(src)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(MergeOutcome {
            dst,
            src,
            events_moved,
        })
    }
}
