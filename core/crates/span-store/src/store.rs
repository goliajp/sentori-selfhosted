//! [`SpanStore`] — the public handle.

use sentori_workspace_identity::ProjectId;
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::Cursor;
use crate::error::SpanStoreError;
use crate::model::{
    ListTraceFilter, PaginatedTraces, Span, SpanInput, SpanStatus, Trace, TraceDetail,
};
use crate::partitions::PartitionLifecycle;

/// Maximum allowed `op` length (chars). Defensive — SDK
/// caps too, but we re-enforce so a buggy SDK can't bloat the
/// table.
const MAX_OP_CHARS: usize = 128;
/// Maximum allowed `name` length.
const MAX_NAME_CHARS: usize = 512;
/// Maximum allowed `traceparent` length.
const MAX_TRACEPARENT_CHARS: usize = 256;

/// Public handle.
#[derive(Debug, Clone)]
pub struct SpanStore {
    pool: PgPool,
}

impl SpanStore {
    /// Construct.
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Borrow the pool.
    #[must_use]
    pub const fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Partition + retention sub-handle. See
    /// [`PartitionLifecycle`].
    #[must_use]
    pub const fn partitions(&self) -> PartitionLifecycle<'_> {
        PartitionLifecycle::new(&self.pool)
    }

    // ── ingest ──────────────────────────────────────────────

    /// Ingest one span. INSERTs the row into `spans` and
    /// UPSERTs the rolled-up `traces` row in a single
    /// transaction.
    ///
    /// Span id is server-minted if the caller passes
    /// `Uuid::nil()`. Returns the persisted span (with
    /// server-set `received_at` + `id`).
    ///
    /// # Errors
    ///
    /// - [`SpanStoreError::InvalidSpan`] for structural
    ///   violations (negative duration, oversized op, etc.).
    /// - [`SpanStoreError::ProjectNotFound`] on FK violation.
    /// - [`SpanStoreError::Db`] on database failure.
    pub async fn ingest_span(
        &self,
        project_id: ProjectId,
        mut input: SpanInput,
    ) -> Result<Span, SpanStoreError> {
        validate_span(&input)?;
        if input.id.is_nil() {
            input.id = Uuid::now_v7();
        }
        let received_at = OffsetDateTime::now_utc();

        let mut tx = self.pool.begin().await?;

        // 1. INSERT the span row. workspace_id denorm via projects
        //    subquery — same pattern as event-pipeline.
        let span_row = sqlx::query(
            r"
            INSERT INTO spans
                (id, workspace_id, project_id, trace_id, parent_span_id, received_at,
                 started_at, duration_ms, op, name, status, tags, data,
                 traceparent)
            SELECT $1, p.workspace_id, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13
            FROM projects p WHERE p.id = $2
            RETURNING id, project_id, trace_id, parent_span_id, received_at,
                      started_at, duration_ms, op, name, status, tags, data,
                      traceparent
            ",
        )
        .bind(input.id)
        .bind(project_id.into_uuid())
        .bind(input.trace_id)
        .bind(input.parent_span_id)
        .bind(received_at)
        .bind(input.started_at)
        .bind(input.duration_ms)
        .bind(&input.op)
        .bind(&input.name)
        .bind(input.status.as_db_str())
        .bind(&input.tags)
        .bind(input.data.as_ref())
        .bind(input.traceparent.as_deref())
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| translate_fk(e, project_id))?;

        // 2. UPSERT the trace row. Root info populates only
        //    when this span IS the root (`parent_span_id IS NULL`);
        //    child spans bump span_count + last_seen + worst-of
        //    status. The worst-of promotion is encoded inline
        //    via a CASE chain — Postgres lacks a built-in.
        let is_root = input.parent_span_id.is_none();
        sqlx::query(
            r"
            INSERT INTO traces
                (trace_id, workspace_id, project_id, root_op, root_name,
                 first_seen, last_seen, span_count, status, duration_ms)
            SELECT $1, p.workspace_id, $2,
                    CASE WHEN $3 THEN $4 ELSE NULL END,
                    CASE WHEN $3 THEN $5 ELSE NULL END,
                    $6, $6, 1, $7,
                    CASE WHEN $3 THEN $8 ELSE 0 END
            FROM projects p WHERE p.id = $2
            ON CONFLICT (trace_id) DO UPDATE SET
                root_op   = COALESCE(traces.root_op,   EXCLUDED.root_op),
                root_name = COALESCE(traces.root_name, EXCLUDED.root_name),
                last_seen = GREATEST(traces.last_seen, EXCLUDED.last_seen),
                first_seen = LEAST(traces.first_seen, EXCLUDED.first_seen),
                span_count = traces.span_count + 1,
                duration_ms = CASE WHEN EXCLUDED.root_op IS NOT NULL
                                   THEN EXCLUDED.duration_ms
                                   ELSE traces.duration_ms
                              END,
                status = CASE
                    WHEN traces.status = 'error' OR EXCLUDED.status = 'error'
                        THEN 'error'
                    WHEN traces.status = 'cancelled' OR EXCLUDED.status = 'cancelled'
                        THEN 'cancelled'
                    ELSE 'ok'
                END
            ",
        )
        .bind(input.trace_id)
        .bind(project_id.into_uuid())
        .bind(is_root)
        .bind(&input.op)
        .bind(&input.name)
        .bind(received_at)
        .bind(input.status.as_db_str())
        .bind(input.duration_ms)
        .execute(&mut *tx)
        .await
        .map_err(|e| translate_fk(e, project_id))?;

        tx.commit().await?;

        crate::model::row_to_span(&span_row)
    }

    // ── read ────────────────────────────────────────────────

    /// Cursor-paginated trace list, filtered.
    ///
    /// # Errors
    ///
    /// [`SpanStoreError::Db`] on database failure.
    pub async fn list_traces(
        &self,
        project_id: ProjectId,
        filter: ListTraceFilter,
        cursor: Cursor,
    ) -> Result<PaginatedTraces, SpanStoreError> {
        let fetch_limit = i64::from(cursor.limit) + 1;
        let (anchor_ts, anchor_id) = match cursor.anchor {
            Some((ts, id)) => (Some(ts), Some(id)),
            None => (None, None),
        };

        let rows = sqlx::query(
            r"
            SELECT trace_id, project_id, root_op, root_name,
                   first_seen, last_seen, span_count, status, duration_ms
            FROM traces
            WHERE project_id = $1
              AND ($2::text IS NULL OR status = $2)
              AND ($3::text IS NULL OR root_op = $3)
              AND ($4::timestamptz IS NULL OR last_seen >= $4)
              AND ($5::int4 IS NULL OR duration_ms >= $5)
              AND (
                    $6::timestamptz IS NULL
                    OR (last_seen, trace_id) < ($6::timestamptz, $7::uuid)
                  )
            ORDER BY last_seen DESC, trace_id DESC
            LIMIT $8
            ",
        )
        .bind(project_id.into_uuid())
        .bind(filter.status.map(SpanStatus::as_db_str))
        .bind(filter.root_op.as_deref())
        .bind(filter.last_seen_after)
        .bind(filter.min_duration_ms)
        .bind(anchor_ts)
        .bind(anchor_id)
        .bind(fetch_limit)
        .fetch_all(&self.pool)
        .await?;

        let mut items: Vec<Trace> = rows
            .iter()
            .map(crate::model::row_to_trace)
            .collect::<Result<_, _>>()?;
        let next = if items.len() as i64 > i64::from(cursor.limit) {
            let _ = items.pop();
            let last = items.last().expect("len > 0 after pop");
            Some(Cursor::next(last.last_seen, last.trace_id, cursor.limit))
        } else {
            None
        };
        Ok(PaginatedTraces { items, next })
    }

    /// Look up one trace + every span under it.
    ///
    /// # Errors
    ///
    /// - [`SpanStoreError::TraceNotFound`] if no `traces`
    ///   row matches.
    /// - [`SpanStoreError::Db`] on database failure.
    pub async fn trace_detail(&self, trace_id: Uuid) -> Result<TraceDetail, SpanStoreError> {
        let trace_row = sqlx::query(
            r"
            SELECT trace_id, project_id, root_op, root_name,
                   first_seen, last_seen, span_count, status, duration_ms
            FROM traces WHERE trace_id = $1
            ",
        )
        .bind(trace_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(SpanStoreError::TraceNotFound(trace_id))?;
        let trace = crate::model::row_to_trace(&trace_row)?;
        let spans = self.spans_for_trace(trace_id).await?;
        Ok(TraceDetail { trace, spans })
    }

    /// All spans belonging to one trace, ordered by
    /// `started_at` ascending then `id` ascending.
    ///
    /// # Errors
    ///
    /// [`SpanStoreError::Db`] on database failure.
    pub async fn spans_for_trace(&self, trace_id: Uuid) -> Result<Vec<Span>, SpanStoreError> {
        let rows = sqlx::query(
            r"
            SELECT id, project_id, trace_id, parent_span_id, received_at,
                   started_at, duration_ms, op, name, status, tags, data,
                   traceparent
            FROM spans
            WHERE trace_id = $1
            ORDER BY started_at ASC, id ASC
            ",
        )
        .bind(trace_id)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(crate::model::row_to_span).collect()
    }
}

fn validate_span(input: &SpanInput) -> Result<(), SpanStoreError> {
    if input.duration_ms < 0 {
        return Err(SpanStoreError::InvalidSpan(
            "duration_ms must be ≥ 0".into(),
        ));
    }
    let op_chars = input.op.chars().count();
    if op_chars == 0 || op_chars > MAX_OP_CHARS {
        return Err(SpanStoreError::InvalidSpan(format!(
            "op must be 1..={MAX_OP_CHARS} chars, got {op_chars}"
        )));
    }
    let name_chars = input.name.chars().count();
    if name_chars == 0 || name_chars > MAX_NAME_CHARS {
        return Err(SpanStoreError::InvalidSpan(format!(
            "name must be 1..={MAX_NAME_CHARS} chars, got {name_chars}"
        )));
    }
    if let Some(tp) = input.traceparent.as_deref()
        && tp.chars().count() > MAX_TRACEPARENT_CHARS
    {
        return Err(SpanStoreError::InvalidSpan("traceparent too long".into()));
    }
    Ok(())
}

fn translate_fk(err: sqlx::Error, project_id: ProjectId) -> SpanStoreError {
    if let sqlx::Error::Database(db_err) = &err
        && db_err.code().as_deref() == Some("23503")
    {
        return SpanStoreError::ProjectNotFound(project_id.into_uuid());
    }
    SpanStoreError::Db(err)
}
