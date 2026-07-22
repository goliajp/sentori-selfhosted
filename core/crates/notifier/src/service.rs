//! [`NotifierService`] — registry + dispatch + delivery_log.

use std::collections::HashMap;
use std::sync::Arc;

use sentori_workspace_identity::ProjectId;
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::{NotifierError, TransportError};
use crate::model::{
    BODY_PREVIEW_BYTES, Channel, DeliveryLog, DeliveryStatus, DispatchOutcome, Notification,
    row_to_log, truncate_body,
};
use crate::transports::Notifier;

/// Public handle.
///
/// Registers transports per [`Channel`] and dispatches every
/// [`Notification`] through the matching transport, persisting
/// the attempt in `delivery_log`.
#[derive(Clone)]
pub struct NotifierService {
    pool: PgPool,
    transports: HashMap<Channel, Arc<dyn Notifier>>,
}

impl std::fmt::Debug for NotifierService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NotifierService")
            .field("pool", &self.pool)
            .field(
                "channels",
                &self.transports.keys().copied().collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl NotifierService {
    /// Construct with an empty transport registry. Call
    /// [`Self::register`] for each channel the consumer
    /// wants.
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            transports: HashMap::new(),
        }
    }

    /// Borrow the pool.
    #[must_use]
    pub const fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Register a transport. Replaces any prior transport
    /// for the same [`Channel`].
    pub fn register(&mut self, transport: Arc<dyn Notifier>) {
        self.transports.insert(transport.channel(), transport);
    }

    /// Channels with a registered transport.
    #[must_use]
    pub fn channels(&self) -> Vec<Channel> {
        self.transports.keys().copied().collect()
    }

    // ── dispatch ────────────────────────────────────────────

    /// One-shot dispatch:
    ///
    /// 1. Validate `n` (non-empty subject/recipient).
    /// 2. Look up transport for `n.channel`; absent →
    ///    [`TransportError::NoTransport`].
    /// 3. INSERT pending row keyed by dedup_key. If dedup
    ///    collides → [`DispatchOutcome::Deduplicated`].
    /// 4. Call transport.
    /// 5. UPDATE row to delivered / failed.
    ///
    /// # Errors
    ///
    /// - [`NotifierError::InvalidInput`] on validation fail.
    /// - [`NotifierError::ProjectNotFound`] on FK violation.
    /// - [`NotifierError::Db`] on database failure.
    ///
    /// Transport failure does NOT propagate as Err — it's
    /// reported via [`DispatchOutcome::Failed`] so the caller
    /// gets the log id regardless.
    pub async fn dispatch(&self, n: &Notification) -> Result<DispatchOutcome, NotifierError> {
        validate(n)?;
        let transport = self.transports.get(&n.channel).cloned().ok_or_else(|| {
            NotifierError::Transport(TransportError::NoTransport {
                channel: n.channel.to_string(),
            })
        })?;

        let id = Uuid::now_v7();
        let body_preview = if n.body.is_empty() {
            None
        } else {
            Some(truncate_body(&n.body))
        };

        let inserted: Option<(Uuid,)> = sqlx::query_as(
            r"
            INSERT INTO delivery_log
                (id, workspace_id, project_id, channel, recipient, subject,
                 body_preview, metadata, status, retries, dedup_key)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'pending', 0, $9)
            ON CONFLICT (workspace_id, dedup_key) WHERE dedup_key IS NOT NULL DO NOTHING
            RETURNING id
            ",
        )
        .bind(id)
        .bind(n.workspace_id.into_uuid())
        .bind(n.project_id.map(ProjectId::into_uuid))
        .bind(n.channel.as_db_str())
        .bind(&n.recipient)
        .bind(&n.subject)
        .bind(body_preview.as_deref())
        .bind(&n.metadata)
        .bind(n.dedup_key.as_deref())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| translate_fk(e, n.project_id))?;

        // dedup conflict → load + return existing row.
        let log_id = match inserted {
            Some((id,)) => id,
            None => {
                // dedup_key collision — fetch the row that's
                // already there.
                let Some(key) = n.dedup_key.as_deref() else {
                    // No dedup key but no insert? Shouldn't
                    // happen — surface as Db error.
                    return Err(NotifierError::Db(sqlx::Error::RowNotFound));
                };
                let existing = self
                    .find_by_dedup(key)
                    .await?
                    .ok_or_else(|| NotifierError::Db(sqlx::Error::RowNotFound))?;
                return Ok(DispatchOutcome::Deduplicated {
                    existing: Box::new(existing),
                });
            }
        };

        match transport.send(n).await {
            Ok(()) => {
                self.mark_delivered(log_id).await?;
                Ok(DispatchOutcome::Delivered { log_id })
            }
            Err(e) => {
                let msg = e.to_log_error();
                self.mark_failed(log_id, &msg).await?;
                Ok(DispatchOutcome::Failed { log_id, error: msg })
            }
        }
    }

    /// Re-dispatch a previously-failed (or pending) log row.
    /// Increments `retries`, resets `status` to pending,
    /// calls transport, records outcome. Idempotent — re-
    /// retrying an already-delivered row is a no-op and
    /// returns the prior [`DispatchOutcome::Delivered`].
    ///
    /// # Errors
    ///
    /// - [`NotifierError::LogNotFound`] when `log_id`
    ///   doesn't exist.
    /// - [`NotifierError::Db`] on database failure.
    /// - [`NotifierError::Transport`] only via the embedded
    ///   `Failed` variant (matches `dispatch` shape).
    pub async fn retry_one(&self, log_id: Uuid) -> Result<DispatchOutcome, NotifierError> {
        let log = self
            .find(log_id)
            .await?
            .ok_or(NotifierError::LogNotFound(log_id))?;
        if log.status == DeliveryStatus::Delivered {
            return Ok(DispatchOutcome::Delivered { log_id });
        }

        let transport = self.transports.get(&log.channel).cloned().ok_or_else(|| {
            NotifierError::Transport(TransportError::NoTransport {
                channel: log.channel.to_string(),
            })
        })?;
        let n = Notification {
            workspace_id: log.workspace_id,
            project_id: log.project_id,
            channel: log.channel,
            recipient: log.recipient.clone(),
            subject: log.subject.clone(),
            // Body persisted only as preview; retry uses the
            // preview as the body for K11 (the K11 design
            // says body retention is short-form preview, not
            // full body). Caller-driven retries with full
            // body should re-call `dispatch`.
            body: log.body_preview.clone().unwrap_or_default(),
            metadata: log.metadata.clone(),
            dedup_key: log.dedup_key.clone(),
        };

        // Bump retries + reset status.
        sqlx::query(
            "UPDATE delivery_log \
             SET retries = retries + 1, status = 'pending', error = NULL \
             WHERE id = $1",
        )
        .bind(log_id)
        .execute(&self.pool)
        .await?;

        match transport.send(&n).await {
            Ok(()) => {
                self.mark_delivered(log_id).await?;
                Ok(DispatchOutcome::Delivered { log_id })
            }
            Err(e) => {
                let msg = e.to_log_error();
                self.mark_failed(log_id, &msg).await?;
                Ok(DispatchOutcome::Failed { log_id, error: msg })
            }
        }
    }

    // ── read ────────────────────────────────────────────────

    /// One log row by id.
    ///
    /// # Errors
    ///
    /// [`NotifierError::Db`] on database failure.
    pub async fn find(&self, log_id: Uuid) -> Result<Option<DeliveryLog>, NotifierError> {
        let row = sqlx::query(SELECT_COLS)
            .bind(log_id)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(row_to_log).transpose()
    }

    /// One log row by dedup_key (None if no row).
    ///
    /// # Errors
    ///
    /// [`NotifierError::Db`] on database failure.
    pub async fn find_by_dedup(
        &self,
        dedup_key: &str,
    ) -> Result<Option<DeliveryLog>, NotifierError> {
        let row = sqlx::query(SELECT_COLS_BY_DEDUP)
            .bind(dedup_key)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(row_to_log).transpose()
    }

    /// Recent log rows for a project, sorted by `created_at`
    /// descending. `since` filters out anything older.
    ///
    /// # Errors
    ///
    /// [`NotifierError::Db`] on database failure.
    pub async fn list_recent(
        &self,
        project_id: ProjectId,
        since: OffsetDateTime,
        limit: u32,
    ) -> Result<Vec<DeliveryLog>, NotifierError> {
        let rows = sqlx::query(
            r"
            SELECT id, workspace_id, project_id, channel, recipient, subject,
                   body_preview, metadata, status, retries, error,
                   dedup_key, sent_at, created_at
            FROM delivery_log
            WHERE project_id = $1 AND created_at >= $2
            ORDER BY created_at DESC
            LIMIT $3
            ",
        )
        .bind(project_id.into_uuid())
        .bind(since)
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_log).collect()
    }

    /// Pending rows older than `older_than`. Caller cron
    /// uses this to find retry candidates.
    ///
    /// # Errors
    ///
    /// [`NotifierError::Db`] on database failure.
    pub async fn list_pending(
        &self,
        older_than: OffsetDateTime,
        limit: u32,
    ) -> Result<Vec<DeliveryLog>, NotifierError> {
        let rows = sqlx::query(
            r"
            SELECT id, workspace_id, project_id, channel, recipient, subject,
                   body_preview, metadata, status, retries, error,
                   dedup_key, sent_at, created_at
            FROM delivery_log
            WHERE status = 'pending' AND created_at < $1
            ORDER BY created_at ASC
            LIMIT $2
            ",
        )
        .bind(older_than)
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_log).collect()
    }

    // ── internals ───────────────────────────────────────────

    async fn mark_delivered(&self, log_id: Uuid) -> Result<(), NotifierError> {
        sqlx::query(
            "UPDATE delivery_log SET status = 'delivered', sent_at = now(), error = NULL \
             WHERE id = $1",
        )
        .bind(log_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn mark_failed(&self, log_id: Uuid, err: &str) -> Result<(), NotifierError> {
        sqlx::query(
            "UPDATE delivery_log SET status = 'failed', error = $2 \
             WHERE id = $1",
        )
        .bind(log_id)
        .bind(err)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

// ── helpers ──────────────────────────────────────────────────

const SELECT_COLS: &str = r"
    SELECT id, workspace_id, project_id, channel, recipient, subject,
           body_preview, metadata, status, retries, error,
           dedup_key, sent_at, created_at
    FROM delivery_log
    WHERE id = $1
";

const SELECT_COLS_BY_DEDUP: &str = r"
    SELECT id, workspace_id, project_id, channel, recipient, subject,
           body_preview, metadata, status, retries, error,
           dedup_key, sent_at, created_at
    FROM delivery_log
    WHERE dedup_key = $1
";

fn validate(n: &Notification) -> Result<(), NotifierError> {
    if n.recipient.trim().is_empty() {
        return Err(NotifierError::InvalidInput(
            "recipient must not be empty".into(),
        ));
    }
    if n.subject.trim().is_empty() {
        return Err(NotifierError::InvalidInput(
            "subject must not be empty".into(),
        ));
    }
    if n.subject.len() > 998 {
        // RFC 5322 §2.1.1 hard limit.
        return Err(NotifierError::InvalidInput(format!(
            "subject too long: {} > 998",
            n.subject.len()
        )));
    }
    // Body preview cap is per-byte — `dispatch` truncates
    // before persisting. We accept any body length at the
    // input boundary.
    let _ = BODY_PREVIEW_BYTES;
    Ok(())
}

fn translate_fk(err: sqlx::Error, project_id: Option<ProjectId>) -> NotifierError {
    if let sqlx::Error::Database(db_err) = &err
        && db_err.code().as_deref() == Some("23503")
        && let Some(pid) = project_id
    {
        return NotifierError::ProjectNotFound(pid.into_uuid());
    }
    NotifierError::Db(err)
}
