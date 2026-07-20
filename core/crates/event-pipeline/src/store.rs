//! Low-level DB access for `issues` + `events` tables.
//!
//! Lives behind [`crate::IngestService`]; not exposed publicly.
//!
//! `pub(super)` is needed so the sibling `pipeline.rs` module
//! can call into here. clippy's `redundant_pub_crate` treats
//! `pub(super)` inside a private module as redundant, but it's
//! load-bearing — bare `fn` would make these unreachable from
//! `pipeline.rs`.
#![allow(clippy::redundant_pub_crate)]

use sentori_workspace_identity::ProjectId;
use sqlx::{PgPool, Row};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::IngestError;
use crate::model::{Event, EventKind, IngestOutcome, Issue, IssueStatus, Platform, StoredEvent};

/// Full ingest write — issue UPSERT + event INSERT in one
/// transaction. Both writes succeed or neither; downstream
/// notifier / alert layers see a consistent snapshot.
///
/// # Errors
///
/// - [`IngestError::ProjectNotFound`] on FK violation.
/// - [`IngestError::Db`] on other database failure.
pub(super) async fn persist_event(
    pool: &PgPool,
    project_id: ProjectId,
    fingerprint: &str,
    event: &Event,
) -> Result<IngestOutcome, IngestError> {
    let mut tx = pool.begin().await?;

    let new_id = Uuid::now_v7();
    let issue_error_type: &str = event.error_type.as_deref().unwrap_or("Message");
    let issue_message_sample: String = event.message.clone().unwrap_or_default();

    // workspace_id is denormalized from projects.workspace_id via
    // subquery — caller's PgPool can be either superuser (janitor)
    // or workspace-scoped (CRUD) without API change.
    let row = sqlx::query(
        r"
        INSERT INTO issues
            (id, workspace_id, project_id, fingerprint, error_type, message_sample, kind,
             status, first_seen, last_seen, event_count,
             last_environment, last_release)
        SELECT $1, p.workspace_id, $2, $3, $4, $5, $6, 'active', $7, $7, 1, $8, $9
        FROM projects p WHERE p.id = $2
        ON CONFLICT (project_id, fingerprint) DO UPDATE SET
            last_seen        = GREATEST(issues.last_seen, EXCLUDED.last_seen),
            event_count      = issues.event_count + 1,
            last_environment = EXCLUDED.last_environment,
            last_release     = EXCLUDED.last_release,
            status           = CASE WHEN issues.status = 'resolved'
                                    THEN 'regressed'
                                    ELSE issues.status
                               END,
            regressed_at     = CASE WHEN issues.status = 'resolved'
                                    THEN EXCLUDED.last_seen
                                    ELSE issues.regressed_at
                               END,
            regressed_in_release = CASE WHEN issues.status = 'resolved'
                                        THEN EXCLUDED.last_release
                                        ELSE issues.regressed_in_release
                                   END
        RETURNING
            id,
            (xmax = 0) AS is_new,
            (xmax <> 0 AND status = 'regressed' AND regressed_at = $7) AS regressed
        ",
    )
    .bind(new_id)
    .bind(project_id.into_uuid())
    .bind(fingerprint)
    .bind(issue_error_type)
    .bind(&issue_message_sample)
    .bind(event.kind.as_db_str())
    .bind(event.timestamp)
    .bind(&event.environment)
    .bind(&event.release)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| translate_fk(e, project_id))?;

    let issue_id: Uuid = row.get("id");
    let is_new: bool = row.get("is_new");
    let regressed: bool = row.get("regressed");

    sqlx::query(
        r"
        INSERT INTO events
            (id, workspace_id, project_id, issue_id, timestamp, kind, platform,
             release, environment, payload)
        SELECT $1, p.workspace_id, $2, $3, $4, $5, $6, $7, $8, $9
        FROM projects p WHERE p.id = $2
        ",
    )
    .bind(event.id)
    .bind(project_id.into_uuid())
    .bind(issue_id)
    .bind(event.timestamp)
    .bind(event.kind.as_db_str())
    .bind(event.platform.as_db_str())
    .bind(&event.release)
    .bind(&event.environment)
    .bind(&event.payload)
    .execute(&mut *tx)
    .await
    .map_err(|e| translate_fk(e, project_id))?;

    tx.commit().await?;

    Ok(IngestOutcome {
        event_id: event.id,
        issue_id,
        is_new_issue: is_new,
        regressed,
    })
}

/// Look up an issue by `(project_id, fingerprint)`. Used by
/// dashboard + tests.
///
/// # Errors
///
/// [`IngestError::Db`] on database failure.
pub(super) async fn find_issue_by_fingerprint(
    pool: &PgPool,
    project_id: ProjectId,
    fingerprint: &str,
) -> Result<Option<Issue>, IngestError> {
    let row = sqlx::query(
        r"
        SELECT id, project_id, fingerprint, error_type, message_sample, kind,
               status, first_seen, last_seen, event_count,
               last_environment, last_release,
               regressed_at, regressed_in_release, resolved_at
        FROM issues
        WHERE project_id = $1 AND fingerprint = $2
        ",
    )
    .bind(project_id.into_uuid())
    .bind(fingerprint)
    .fetch_optional(pool)
    .await?;
    row.as_ref().map(row_to_issue).transpose()
}

/// Look up an issue by id.
///
/// # Errors
///
/// [`IngestError::Db`] on database failure.
pub(super) async fn find_issue(
    pool: &PgPool,
    issue_id: Uuid,
) -> Result<Option<Issue>, IngestError> {
    let row = sqlx::query(
        r"
        SELECT id, project_id, fingerprint, error_type, message_sample, kind,
               status, first_seen, last_seen, event_count,
               last_environment, last_release,
               regressed_at, regressed_in_release, resolved_at
        FROM issues
        WHERE id = $1
        ",
    )
    .bind(issue_id)
    .fetch_optional(pool)
    .await?;
    row.as_ref().map(row_to_issue).transpose()
}

/// Look up a stored event by id.
///
/// # Errors
///
/// [`IngestError::Db`] on database failure.
pub(super) async fn find_event(
    pool: &PgPool,
    event_id: Uuid,
) -> Result<Option<StoredEvent>, IngestError> {
    let row = sqlx::query(
        r"
        SELECT id, project_id, issue_id, timestamp, kind, platform,
               release, environment, payload, received_at
        FROM events
        WHERE id = $1
        ",
    )
    .bind(event_id)
    .fetch_optional(pool)
    .await?;
    row.as_ref().map(row_to_event).transpose()
}

/// Count events under a given issue. Cheap aggregate; not
/// pulled into [`Issue`] because the row's `event_count` is
/// the authoritative figure.
///
/// # Errors
///
/// [`IngestError::Db`] on database failure.
pub(super) async fn count_events_for_issue(
    pool: &PgPool,
    issue_id: Uuid,
) -> Result<i64, IngestError> {
    let row = sqlx::query("SELECT count(*)::bigint AS n FROM events WHERE issue_id = $1")
        .bind(issue_id)
        .fetch_one(pool)
        .await?;
    Ok(row.get::<i64, _>("n"))
}

/// Flip an issue's status (used by tests to set up
/// regression scenarios + by the consumer crate's dashboard
/// mutate endpoint).
///
/// # Errors
///
/// [`IngestError::Db`] on database failure.
pub(super) async fn set_issue_status(
    pool: &PgPool,
    issue_id: Uuid,
    status: IssueStatus,
    resolved_at: Option<OffsetDateTime>,
) -> Result<(), IngestError> {
    sqlx::query(
        r"
        UPDATE issues
        SET status = $1,
            resolved_at = $2
        WHERE id = $3
        ",
    )
    .bind(status.as_db_str())
    .bind(resolved_at)
    .bind(issue_id)
    .execute(pool)
    .await?;
    Ok(())
}

fn row_to_issue(row: &sqlx::postgres::PgRow) -> Result<Issue, IngestError> {
    let kind_str: &str = row.get("kind");
    let status_str: &str = row.get("status");
    Ok(Issue {
        id: row.get("id"),
        project_id: ProjectId::from_uuid(row.get("project_id")),
        fingerprint: row.get("fingerprint"),
        error_type: row.get("error_type"),
        message_sample: row.get("message_sample"),
        kind: EventKind::from_db_str(kind_str)?,
        status: IssueStatus::from_db_str(status_str)?,
        first_seen: row.get("first_seen"),
        last_seen: row.get("last_seen"),
        event_count: row.get("event_count"),
        last_environment: row.get("last_environment"),
        last_release: row.get("last_release"),
        regressed_at: row.get("regressed_at"),
        regressed_in_release: row.get("regressed_in_release"),
        resolved_at: row.get("resolved_at"),
    })
}

fn row_to_event(row: &sqlx::postgres::PgRow) -> Result<StoredEvent, IngestError> {
    let kind_str: &str = row.get("kind");
    let platform_str: &str = row.get("platform");
    Ok(StoredEvent {
        id: row.get("id"),
        project_id: ProjectId::from_uuid(row.get("project_id")),
        issue_id: row.get("issue_id"),
        timestamp: row.get("timestamp"),
        kind: EventKind::from_db_str(kind_str)?,
        platform: Platform::from_db_str(platform_str)?,
        release: row.get("release"),
        environment: row.get("environment"),
        payload: row.get("payload"),
        received_at: row.get("received_at"),
    })
}

pub(super) fn translate_fk(err: sqlx::Error, project_id: ProjectId) -> IngestError {
    if let sqlx::Error::Database(db_err) = &err {
        // 23503 = foreign_key_violation.
        if db_err.code().as_deref() == Some("23503") {
            return IngestError::ProjectNotFound(project_id.into_uuid());
        }
    }
    IngestError::Db(err)
}
