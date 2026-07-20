-- Sentori core migration 0022 — event_attachments + dsyms + proguard_mappings.
--
-- `event_attachments` is the metadata row for every binary
-- the SDK posts alongside an event:
--   • screenshot     — PNG/JPEG from `captureWireframe`
--   • viewTree       — UI tree JSON
--   • stateSnapshot  — redux/zustand store dump
--   • logTail        — last N log lines
--   • sessionTrail   — breadcrumbs + nav timeline
--   • replay         — replay-event log fragment
-- Bytes live in the configured object store keyed by `ref`.
--
-- `dsyms` + `proguard_mappings` are the deobfuscation blobs
-- the symbolicator loads at issue render time. Their bytes are
-- inline `BYTEA` (legacy decision — small enough to keep in
-- Postgres); the SaaS object-store migration is a later phase.
--
-- Columns verbatim from legacy
-- `server/migrations/0032_event_attachments.sql` +
-- `0035_event_attachments_session_trail.sql` +
-- `0043_attachments_replay_kind.sql` +
-- `0014_dsyms.sql` + `0015_proguard_mappings.sql`.

CREATE TABLE IF NOT EXISTS event_attachments (
    ref          UUID        PRIMARY KEY,
    workspace_id UUID        NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id   UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    event_id     UUID        NOT NULL,
    kind         TEXT        NOT NULL CHECK (kind IN ('screenshot', 'viewTree', 'stateSnapshot', 'logTail', 'sessionTrail', 'replay')),
    media_type   TEXT        NOT NULL,
    size_bytes   INTEGER     NOT NULL,
    captured_at  TIMESTAMPTZ NOT NULL,
    source       TEXT        NOT NULL CHECK (source IN ('js', 'ios', 'android')),
    received_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS event_attachments_event_id_idx
    ON event_attachments (event_id);
CREATE INDEX IF NOT EXISTS event_attachments_received_at_idx
    ON event_attachments (received_at);

CREATE TABLE IF NOT EXISTS dsyms (
    id           UUID        PRIMARY KEY,
    workspace_id UUID        NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id   UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    release      TEXT,
    debug_id     TEXT        NOT NULL,
    arch         TEXT        NOT NULL,
    object_name  TEXT,
    size_bytes   INTEGER     NOT NULL,
    data         BYTEA       NOT NULL,
    uploaded_by  UUID        REFERENCES users(id) ON DELETE SET NULL,
    uploaded_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX IF NOT EXISTS dsyms_lookup_idx
    ON dsyms (project_id, debug_id, arch);
CREATE INDEX IF NOT EXISTS dsyms_project_release_idx
    ON dsyms (project_id, release, uploaded_at DESC);

CREATE TABLE IF NOT EXISTS proguard_mappings (
    id           UUID        PRIMARY KEY,
    workspace_id UUID        NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id   UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    release      TEXT,
    debug_id     TEXT,
    size_bytes   INTEGER     NOT NULL,
    data         BYTEA       NOT NULL,
    uploaded_by  UUID        REFERENCES users(id) ON DELETE SET NULL,
    uploaded_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS proguard_mappings_debug_idx
    ON proguard_mappings (project_id, debug_id);
CREATE INDEX IF NOT EXISTS proguard_mappings_release_idx
    ON proguard_mappings (project_id, release, uploaded_at DESC);
