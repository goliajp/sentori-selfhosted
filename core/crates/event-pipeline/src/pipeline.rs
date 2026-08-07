//! [`IngestService`] — the public handle.

use sentori_event_ringbuffer::{PushOutcome, Ring};
use sentori_issue_fingerprint::{Fingerprint, FrameSite, Input};
use sentori_workspace_identity::ProjectId;
use sqlx::PgPool;

use crate::error::IngestError;
use crate::model::{EnqueuedEvent, Event, EventKind, IngestOutcome, Issue, StoredEvent};
use crate::store;

/// Tuning knobs for [`IngestService`].
#[derive(Debug, Clone, Copy)]
pub struct IngestOptions {
    /// Ring buffer capacity. Bounded — older events are
    /// drop-oldest evicted under burst per S4's `Ring`. Default
    /// 4096 (≈ 60 s of traffic at 60 evt/s).
    pub ring_capacity: usize,
}

impl Default for IngestOptions {
    fn default() -> Self {
        Self {
            ring_capacity: 4096,
        }
    }
}

/// Public ingest entry point.
///
/// Holds:
/// - the database pool (cheap to clone — `Arc` inside);
/// - the bounded ring buffer used by `try_enqueue` /
///   `flush` (S4-backed; cloning the service clones the ring
///   handle so producers + the flusher share state);
/// - the tuning knobs.
///
/// Cheap to clone; both `PgPool` and `Ring` are internally
/// `Arc`-shared.
#[derive(Clone)]
pub struct IngestService {
    pool: PgPool,
    ring: Ring<EnqueuedEvent>,
    opts: IngestOptions,
}

impl std::fmt::Debug for IngestService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IngestService")
            .field("pool", &self.pool)
            .field("opts", &self.opts)
            .field("ring_len", &self.ring.len())
            .field("ring_capacity", &self.ring.capacity())
            .field("ring_dropped", &self.ring.dropped_count())
            .finish()
    }
}

impl IngestService {
    /// Construct.
    ///
    /// # Errors
    ///
    /// Returns [`IngestError::InvalidEvent`] if
    /// `opts.ring_capacity == 0` (the underlying S4 ring
    /// requires capacity ≥ 1).
    pub fn new(pool: PgPool, opts: IngestOptions) -> Result<Self, IngestError> {
        let ring = Ring::with_capacity(opts.ring_capacity)
            .map_err(|e| IngestError::InvalidEvent(format!("ring capacity invalid: {e}")))?;
        Ok(Self { pool, ring, opts })
    }

    /// Borrow the pool. Exposed for the consumer's ad-hoc
    /// queries on the same db.
    #[must_use]
    pub const fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Borrow the ring buffer. Exposed for telemetry / janitor
    /// `dropped_count()` snapshots.
    #[must_use]
    pub const fn ringbuffer(&self) -> &Ring<EnqueuedEvent> {
        &self.ring
    }

    /// Tuning knobs.
    #[must_use]
    pub const fn opts(&self) -> &IngestOptions {
        &self.opts
    }

    /// Compute the fingerprint that [`Self::ingest`] would
    /// assign. Pure — no DB I/O. Useful for "show me what this
    /// would group as" dashboards.
    #[must_use]
    pub fn fingerprint(event: &Event) -> String {
        if let Some(override_fp) = event.fingerprint_override.as_deref() {
            // S3's `from_override` runs format validation; if
            // the override fails it (empty / too long / control
            // chars) fall through to the algorithmic
            // fingerprint to keep ingest going. Logged at
            // ingest time by the caller.
            if let Ok(fp) = Fingerprint::from_override(override_fp) {
                return fp.into_hex();
            }
        }

        let input = build_fingerprint_input(event);
        Fingerprint::compute(&input).into_hex()
    }

    /// Write-through ingest. Validates, fingerprints, UPSERTs
    /// the issue, persists the event row — all in one
    /// transaction.
    ///
    /// Returns the issue id + whether the issue is new +
    /// whether the UPSERT atomically flipped a previously
    /// resolved issue back to regressed.
    ///
    /// # Errors
    ///
    /// See [`IngestError`].
    pub async fn ingest(
        &self,
        project_id: ProjectId,
        event: Event,
    ) -> Result<IngestOutcome, IngestError> {
        validate_event(&event)?;
        let fp = Self::fingerprint(&event);
        store::persist_event(&self.pool, project_id, &fp, &event).await
    }

    /// Push the event into the ring buffer. Returns the
    /// underlying [`PushOutcome`] so callers can split
    /// telemetry by inserted vs evicted vs dropped.
    ///
    /// Does NOT persist — call [`Self::flush`] (or run a
    /// caller-owned tokio task that does) to drain the buffer
    /// to the DB.
    ///
    /// # Errors
    ///
    /// - [`IngestError::InvalidEvent`] on structural failure.
    /// - [`IngestError::RingDropped`] when the push lost the
    ///   race to fit. The ring's atomic drop counter also
    ///   ticks in this case; the consumer can choose to log
    ///   the counter periodically instead of per-call.
    pub fn try_enqueue(
        &self,
        project_id: ProjectId,
        event: Event,
    ) -> Result<PushOutcome, IngestError> {
        validate_event(&event)?;
        let outcome = self.ring.push(EnqueuedEvent { project_id, event });
        if matches!(outcome, PushOutcome::Dropped) {
            return Err(IngestError::RingDropped);
        }
        Ok(outcome)
    }

    /// Drain the ring buffer to the DB. Returns the number of
    /// events successfully persisted.
    ///
    /// Per-event failures are logged via [`tracing::warn`] but
    /// do NOT abort the drain — one bad event must not stall
    /// the buffer. The returned count is best-effort: it
    /// reflects successes; failures are observable via the
    /// log.
    ///
    /// Caller pattern (in the saas/server / self-hosted/server
    /// bin):
    ///
    /// ```ignore
    /// let svc = svc.clone();
    /// tokio::spawn(async move {
    ///     loop {
    ///         tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    ///         if let Err(e) = svc.flush().await {
    ///             tracing::error!(error = %e, "ingest flush failed");
    ///         }
    ///     }
    /// });
    /// ```
    ///
    /// # Errors
    ///
    /// Currently never errors — a panic-free flush is the
    /// contract. The `Result` shape leaves room for a future
    /// fatal-batch failure mode without breaking call sites.
    pub async fn flush(&self) -> Result<usize, IngestError> {
        let mut persisted = 0usize;
        while let Some(EnqueuedEvent { project_id, event }) = self.ring.pop() {
            // Re-validate in case the event was queued before a
            // newer set of validation rules took effect — cheap
            // belt-and-braces.
            if let Err(e) = validate_event(&event) {
                tracing::warn!(error = %e, event_id = %event.id, "drop invalid queued event");
                continue;
            }
            let fp = Self::fingerprint(&event);
            match store::persist_event(&self.pool, project_id, &fp, &event).await {
                Ok(_) => persisted += 1,
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        event_id = %event.id,
                        %project_id,
                        "ingest persist failed (event dropped from flush)",
                    );
                }
            }
        }
        Ok(persisted)
    }

    /// Look up an issue by its hash key. Convenience wrapper
    /// around the internal store; consumer crates can also
    /// query the DB directly.
    ///
    /// # Errors
    ///
    /// [`IngestError::Db`] on database failure.
    pub async fn find_issue_by_fingerprint(
        &self,
        project_id: ProjectId,
        fingerprint: &str,
    ) -> Result<Option<Issue>, IngestError> {
        store::find_issue_by_fingerprint(&self.pool, project_id, fingerprint).await
    }

    /// Look up an issue by id.
    ///
    /// # Errors
    ///
    /// [`IngestError::Db`] on database failure.
    pub async fn find_issue(&self, issue_id: uuid::Uuid) -> Result<Option<Issue>, IngestError> {
        store::find_issue(&self.pool, issue_id).await
    }

    /// Look up a stored event by id.
    ///
    /// # Errors
    ///
    /// [`IngestError::Db`] on database failure.
    pub async fn find_event(
        &self,
        event_id: uuid::Uuid,
    ) -> Result<Option<StoredEvent>, IngestError> {
        store::find_event(&self.pool, event_id).await
    }

    /// Count persisted events under an issue. The `Issue` row's
    /// `event_count` is authoritative for fast UI reads; this
    /// is for tests + low-frequency reconciliation.
    ///
    /// # Errors
    ///
    /// [`IngestError::Db`] on database failure.
    pub async fn count_events_for_issue(&self, issue_id: uuid::Uuid) -> Result<i64, IngestError> {
        store::count_events_for_issue(&self.pool, issue_id).await
    }

    /// Mutate an issue's lifecycle status. The K-tier dashboard
    /// "Resolve" / "Reopen" / "Ignore" button bottoms out here.
    ///
    /// Pass `resolved_at = Some(now)` when transitioning to
    /// `Resolved`; pass `None` for other transitions.
    ///
    /// # Errors
    ///
    /// [`IngestError::Db`] on database failure.
    pub async fn set_issue_status(
        &self,
        issue_id: uuid::Uuid,
        status: crate::model::IssueStatus,
        resolved_at: Option<time::OffsetDateTime>,
    ) -> Result<(), IngestError> {
        store::set_issue_status(&self.pool, issue_id, status, resolved_at).await
    }
}

/// Map an [`Event`] to the [`Input`] enum [`sentori_issue_fingerprint`]
/// expects.
fn build_fingerprint_input(event: &Event) -> Input<'_> {
    match event.kind {
        EventKind::Message => Input::Message {
            release: &event.release,
            body: event.message.as_deref().unwrap_or(""),
        },
        EventKind::Error | EventKind::Anr | EventKind::NearCrash => {
            if event.error_type.is_some() || event.message.is_some() {
                Input::Exception {
                    release: &event.release,
                    error_type: event.error_type.as_deref().unwrap_or(""),
                    message: event.message.as_deref().unwrap_or(""),
                    frame: event.frame.as_ref().map(|f| FrameSite {
                        function: f.function.as_deref(),
                        file: &f.file,
                    }),
                }
            } else {
                Input::Degenerate {
                    release: &event.release,
                    kind_tag: event.kind.fingerprint_tag(),
                    seed: event.timestamp.unix_timestamp(),
                }
            }
        }
    }
}

/// Structural validation. Cross-field rules `validator` can't
/// express declaratively.
fn validate_event(event: &Event) -> Result<(), IngestError> {
    if event.release.trim().is_empty() {
        return Err(IngestError::InvalidEvent("release is empty".into()));
    }
    if event.release.len() > 200 {
        return Err(IngestError::InvalidEvent(format!(
            "release too long: {}",
            event.release.len()
        )));
    }
    if event.environment.trim().is_empty() {
        return Err(IngestError::InvalidEvent("environment is empty".into()));
    }
    if event.environment.len() > 64 {
        return Err(IngestError::InvalidEvent(format!(
            "environment too long: {}",
            event.environment.len()
        )));
    }
    match event.kind {
        EventKind::Message => {
            if event.message.as_deref().is_none_or(str::is_empty) {
                return Err(IngestError::InvalidEvent(
                    "message-kind requires non-empty message body".into(),
                ));
            }
            if event.level.is_none() {
                return Err(IngestError::InvalidEvent(
                    "message-kind requires level".into(),
                ));
            }
        }
        EventKind::Error | EventKind::Anr | EventKind::NearCrash => {
            // Exception-shape requires at least one identifier
            // (type or message) for the fingerprint to be
            // stable across events of the same issue. If both
            // are missing the pipeline still ingests but
            // routes through the Degenerate fingerprint
            // variant.
            if event.error_type.as_deref().is_some_and(|s| s.len() > 256) {
                return Err(IngestError::InvalidEvent("error_type too long".into()));
            }
            if event.message.as_deref().is_some_and(|s| s.len() > 4096) {
                return Err(IngestError::InvalidEvent("message too long".into()));
            }
        }
    }
    // Fingerprint override length sanity — S3's `from_override`
    // enforces hard limits; we surface a clean error here.
    if let Some(fp) = event.fingerprint_override.as_deref()
        && fp.len() > 200
    {
        return Err(IngestError::InvalidEvent(
            "fingerprint override too long".into(),
        ));
    }
    Ok(())
}
