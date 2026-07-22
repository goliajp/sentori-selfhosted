-- Sentori core migration 0035 — give event_attachments a blob pointer.
--
-- 0022 created `event_attachments` with metadata only (ref, kind,
-- media_type, size_bytes, captured_at, source) and no column pointing
-- at the stored bytes. The blob backend is content-addressed
-- (`BlobStore::put(bytes) -> BlobHash`, `get(&BlobHash) -> bytes`), so
-- without the hash a stored attachment is unreachable the moment the
-- request ends: the screenshot / viewTree / stateSnapshot / logTail /
-- sessionTrail / replay bytes go in and can never come out.
--
-- The ingest handler had always *tried* to write `blob_hash` — the
-- column simply never existed, which is one of the reasons every
-- attachment INSERT failed at runtime and the table sat empty. This
-- adds the column the handler already assumed.
--
-- NOT NULL with no default is safe: the table is empty everywhere,
-- precisely because no INSERT ever succeeded.
ALTER TABLE event_attachments
    ADD COLUMN IF NOT EXISTS blob_hash TEXT NOT NULL;

-- Fetching every attachment for one event is the crash-detail read
-- path; it runs on every issue view.
CREATE INDEX IF NOT EXISTS idx_event_attachments_event
    ON event_attachments (event_id);
