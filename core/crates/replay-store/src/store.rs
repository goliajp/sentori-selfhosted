//! [`ReplayStore`] — the public handle.

use std::io::{Read, Write};
use std::sync::Arc;

use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use sentori_attachment_store::{BlobHash, BlobStore};
use sentori_workspace_identity::ProjectId;
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::Cursor;
use crate::error::ReplayStoreError;
use crate::model::{PaginatedReplays, ReplaySession, row_to_session};
use crate::scrubber::Scrubber;

/// Public handle.
///
/// Generic over `B: BlobStore` because K3's
/// [`sentori_attachment_store::BlobStore`] uses
/// async-fn-in-trait and is intentionally NOT object-safe
/// (per K3's design decision). Consumer crates pick the
/// concrete impl (`LocalFsBlobStore` in prod / `MemoryBlobStore`
/// in tests) and wire that type through.
///
/// The `Arc<B>` wrapper keeps the store cheap to clone — a
/// dispatcher / web handler can hand out clones without
/// reseating the underlying connection pool or blob store.
pub struct ReplayStore<B: BlobStore> {
    pool: PgPool,
    blob_store: Arc<B>,
    scrubber: Scrubber,
}

impl<B: BlobStore> Clone for ReplayStore<B> {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
            blob_store: self.blob_store.clone(),
            scrubber: self.scrubber.clone(),
        }
    }
}

impl<B: BlobStore> std::fmt::Debug for ReplayStore<B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // BlobStore isn't Debug-bounded (the K3 trait doesn't
        // require it; impls happen to be Debug). Skip the
        // blob_store field rather than tighten the bound.
        f.debug_struct("ReplayStore")
            .field("pool", &self.pool)
            .field("blob_store", &"<BlobStore>")
            .field("scrubber_patterns", &self.scrubber.pattern_count())
            .finish()
    }
}

impl<B: BlobStore> ReplayStore<B> {
    /// Construct from an owned blob store. Wraps internally
    /// in `Arc<B>` so `clone()` doesn't dup the backend.
    #[must_use]
    pub fn new(pool: PgPool, blob_store: B, scrubber: Scrubber) -> Self {
        Self {
            pool,
            blob_store: Arc::new(blob_store),
            scrubber,
        }
    }

    /// Construct from a pre-shared `Arc<B>`. Useful when
    /// multiple K-tier crates share one blob store (the K3
    /// LocalFsBlobStore is typically a single instance per
    /// process).
    #[must_use]
    pub const fn from_arc(pool: PgPool, blob_store: Arc<B>, scrubber: Scrubber) -> Self {
        Self {
            pool,
            blob_store,
            scrubber,
        }
    }

    /// Borrow the pool.
    #[must_use]
    pub const fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Borrow the configured scrubber.
    #[must_use]
    pub const fn scrubber(&self) -> &Scrubber {
        &self.scrubber
    }

    // ── store ───────────────────────────────────────────────

    /// Persist a replay session.
    ///
    /// Pipeline:
    /// 1. Scrub the NDJSON (write-time PII redaction).
    /// 2. gzip the scrubbed bytes.
    /// 3. PUT into K3 attachment-store; receive
    ///    [`BlobHash`] (SHA-256).
    /// 4. INSERT `replay_sessions` row with `blob_hash`,
    ///    counts, byte size.
    ///
    /// # Errors
    ///
    /// - [`ReplayStoreError::ProjectNotFound`] /
    ///   [`ReplayStoreError::EventNotFound`] on FK violation.
    /// - [`ReplayStoreError::InvalidInput`] for
    ///   `started_at > ended_at`.
    /// - [`ReplayStoreError::Scrub`] on regex failure.
    /// - [`ReplayStoreError::Compression`] on gzip failure.
    /// - [`ReplayStoreError::Blob`] on K3 backend failure.
    /// - [`ReplayStoreError::Db`] on DB failure.
    pub async fn store(
        &self,
        project_id: ProjectId,
        event_id: Uuid,
        ndjson_bytes: &[u8],
        started_at: OffsetDateTime,
        ended_at: OffsetDateTime,
    ) -> Result<ReplaySession, ReplayStoreError> {
        if started_at > ended_at {
            return Err(ReplayStoreError::InvalidInput(
                "started_at must be ≤ ended_at".into(),
            ));
        }

        // 1. Scrub.
        let (scrubbed, report) = self.scrubber.scrub(ndjson_bytes)?;

        // 2. gzip.
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&scrubbed)?;
        let gzipped = encoder.finish()?;
        let byte_count = i32::try_from(gzipped.len()).unwrap_or(i32::MAX);

        // 3. K3 put — content-addressed.
        let hash = self.blob_store.put(&gzipped).await?;
        let blob_hash_hex = hash.to_hex();

        // 4. Insert metadata row.
        let id = Uuid::now_v7();
        // workspace_id denorm via projects subquery — same pattern
        // as cert-monitor / event-pipeline.
        let row = sqlx::query(
            r"
            INSERT INTO replay_sessions
                (id, workspace_id, project_id, event_id, blob_hash, started_at, ended_at,
                 frame_count, scrubbed_count, byte_count)
            SELECT $1, p.workspace_id, $2, $3, $4, $5, $6, $7, $8, $9
            FROM projects p WHERE p.id = $2
            RETURNING id, project_id, event_id, blob_hash,
                      started_at, ended_at, frame_count, scrubbed_count,
                      byte_count, created_at
            ",
        )
        .bind(id)
        .bind(project_id.into_uuid())
        .bind(event_id)
        .bind(&blob_hash_hex)
        .bind(started_at)
        .bind(ended_at)
        .bind(report.frame_count)
        .bind(report.redaction_count)
        .bind(byte_count)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| translate_fk(e, project_id, event_id))?;

        // A missing event_id trips a real FK violation, but a missing project
        // just makes the driving SELECT match zero rows — nothing is inserted
        // and `translate_fk` never sees a 23503. Absence of a RETURNING row is
        // the only signal.
        let row = row.ok_or_else(|| ReplayStoreError::ProjectNotFound(project_id.into_uuid()))?;

        row_to_session(&row)
    }

    // ── read ────────────────────────────────────────────────

    /// Load + gunzip a session's bytes.
    ///
    /// Returns the raw scrubbed NDJSON — already PII-clean.
    ///
    /// # Errors
    ///
    /// - [`ReplayStoreError::SessionNotFound`] if no row.
    /// - [`ReplayStoreError::Blob`] if the blob is missing
    ///   from K3 (orphaned metadata — janitor should clean).
    /// - [`ReplayStoreError::Compression`] on gunzip failure.
    /// - [`ReplayStoreError::Db`] on DB failure.
    pub async fn fetch(&self, session_id: Uuid) -> Result<Vec<u8>, ReplayStoreError> {
        let session = self
            .find(session_id)
            .await?
            .ok_or(ReplayStoreError::SessionNotFound(session_id))?;
        let hash = BlobHash::from_hex(&session.blob_hash).map_err(|e| {
            ReplayStoreError::InvalidInput(format!("malformed blob_hash in DB: {e}"))
        })?;
        let gzipped = self.blob_store.get(&hash).await?;

        let mut decoder = GzDecoder::new(&gzipped[..]);
        let mut out = Vec::with_capacity(gzipped.len() * 4);
        decoder.read_to_end(&mut out)?;
        Ok(out)
    }

    /// Look up one session by id.
    ///
    /// # Errors
    ///
    /// [`ReplayStoreError::Db`] on database failure.
    pub async fn find(&self, session_id: Uuid) -> Result<Option<ReplaySession>, ReplayStoreError> {
        let row = sqlx::query(
            r"
            SELECT id, project_id, event_id, blob_hash,
                   started_at, ended_at, frame_count, scrubbed_count,
                   byte_count, created_at
            FROM replay_sessions
            WHERE id = $1
            ",
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(row_to_session).transpose()
    }

    /// List every replay session for one event, ordered by
    /// `created_at` descending.
    ///
    /// # Errors
    ///
    /// [`ReplayStoreError::Db`] on database failure.
    pub async fn list_for_event(
        &self,
        event_id: Uuid,
    ) -> Result<Vec<ReplaySession>, ReplayStoreError> {
        let rows = sqlx::query(
            r"
            SELECT id, project_id, event_id, blob_hash,
                   started_at, ended_at, frame_count, scrubbed_count,
                   byte_count, created_at
            FROM replay_sessions
            WHERE event_id = $1
            ORDER BY created_at DESC
            ",
        )
        .bind(event_id)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_session).collect()
    }

    /// Cursor-paginated session list for a project.
    ///
    /// # Errors
    ///
    /// [`ReplayStoreError::Db`] on database failure.
    pub async fn list_for_project(
        &self,
        project_id: ProjectId,
        cursor: Cursor,
    ) -> Result<PaginatedReplays, ReplayStoreError> {
        let fetch_limit = i64::from(cursor.limit) + 1;
        let (anchor_ts, anchor_id) = match cursor.anchor {
            Some((ts, id)) => (Some(ts), Some(id)),
            None => (None, None),
        };
        let rows = sqlx::query(
            r"
            SELECT id, project_id, event_id, blob_hash,
                   started_at, ended_at, frame_count, scrubbed_count,
                   byte_count, created_at
            FROM replay_sessions
            WHERE project_id = $1
              AND (
                    $2::timestamptz IS NULL
                    OR (created_at, id) < ($2::timestamptz, $3::uuid)
                  )
            ORDER BY created_at DESC, id DESC
            LIMIT $4
            ",
        )
        .bind(project_id.into_uuid())
        .bind(anchor_ts)
        .bind(anchor_id)
        .bind(fetch_limit)
        .fetch_all(&self.pool)
        .await?;

        let mut items: Vec<ReplaySession> =
            rows.iter().map(row_to_session).collect::<Result<_, _>>()?;
        let next = if items.len() as i64 > i64::from(cursor.limit) {
            let _ = items.pop();
            let last = items.last().expect("len > 0 after pop");
            Some(Cursor::next(last.created_at, last.id, cursor.limit))
        } else {
            None
        };
        Ok(PaginatedReplays { items, next })
    }

    /// Delete a session row. Blob bytes remain in K3 — a
    /// janitor reaps orphans across all `*_blob_hash` ref
    /// tables periodically. Idempotent (no error if row
    /// is missing).
    ///
    /// # Errors
    ///
    /// [`ReplayStoreError::Db`] on database failure.
    pub async fn delete(&self, session_id: Uuid) -> Result<(), ReplayStoreError> {
        sqlx::query("DELETE FROM replay_sessions WHERE id = $1")
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

fn translate_fk(err: sqlx::Error, project_id: ProjectId, event_id: Uuid) -> ReplayStoreError {
    if let sqlx::Error::Database(db_err) = &err
        && db_err.code().as_deref() == Some("23503")
    {
        // FK 23503 — could be either project or event. The
        // constraint name disambiguates.
        let on_event = db_err.constraint().is_some_and(|c| c.contains("event"));
        return if on_event {
            ReplayStoreError::EventNotFound(event_id)
        } else {
            ReplayStoreError::ProjectNotFound(project_id.into_uuid())
        };
    }
    ReplayStoreError::Db(err)
}
