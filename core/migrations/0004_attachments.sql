-- Sentori v1 migration 0004 — event attachments.
--
-- Metadata rows; bytes live in the content-addressed blob store
-- (attachment-store crate) keyed by blob_hash. B-type replay is an
-- attachment (kind = 'replay') on the event that triggered the
-- upload — there is no replay_sessions table in v1 (design.md §5):
-- a replay only exists because an error/warn happened, so the event
-- is its natural parent.
--
-- No FK to events(id): the attachment upload races the event insert
-- (the SDK posts them separately, and the crash-pending drain can
-- deliver the attachment first). Orphans are bounded by retention.

CREATE TABLE event_attachments (
    ref         UUID        PRIMARY KEY,
    project_id  UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    event_id    UUID        NOT NULL,
    kind        TEXT        NOT NULL
                            CHECK (kind IN ('replay', 'screenshot', 'viewTree',
                                            'stateSnapshot', 'logTail')),
    media_type  TEXT        NOT NULL,
    size_bytes  INTEGER     NOT NULL,
    blob_hash   TEXT        NOT NULL,
    source      TEXT        NOT NULL CHECK (source IN ('js', 'ios', 'android')),
    captured_at TIMESTAMPTZ NOT NULL,
    received_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX event_attachments_event_idx ON event_attachments (event_id);
CREATE INDEX event_attachments_received_idx ON event_attachments (received_at);
