-- Sentori core migration 0007 — replay-store (K8).
--
-- One table: replay_sessions. One row per captured replay
-- session (typically one per captureException with replay
-- enabled, but the wire allows many — long-running session
-- with periodic flush).
--
-- The actual blob bytes live in K3 attachment-store keyed
-- by `blob_hash` (SHA-256 hex of the gzipped scrubbed
-- NDJSON). K8 stores only the metadata + hash; K3 owns the
-- bytes. Deletion is two-step:
--   1. DELETE FROM replay_sessions … (FK cascade from event)
--   2. Janitor GC's orphan blobs in K3 (computes ref set
--      from this table + others, drops blobs not in any).
--
-- Per user decision 2026-06-20, K8 scrubs at write-time:
-- `scrubbed_count` records how many text-node values got
-- redacted, so the dashboard can flag "this session contained
-- N PII matches" without re-scanning.

-- ── multi-tenancy ──────────────────────────────────────────
-- workspace_id NOT NULL denormalized from projects.workspace_id
-- at INSERT. RLS enforces isolation.

CREATE TABLE IF NOT EXISTS replay_sessions (
    id              UUID        PRIMARY KEY,
    workspace_id    UUID        NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id      UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    -- The event that triggered this replay capture. Cascade
    -- delete keeps replay rows lockstep with events partition
    -- drops.
    event_id        UUID        NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    -- Hex SHA-256 of the gzipped scrubbed bytes (64 chars).
    blob_hash       TEXT        NOT NULL,
    -- Replay window time bounds. SDK-supplied; clipped to
    -- (event.timestamp - 60s, event.timestamp + 5s) by the
    -- ingest layer.
    started_at      TIMESTAMPTZ NOT NULL,
    ended_at        TIMESTAMPTZ NOT NULL,
    -- Number of NDJSON frames in the session (keyframes +
    -- deltas). Caps dashboard "this session is X frames"
    -- labels without re-parsing.
    frame_count     INTEGER     NOT NULL,
    -- Number of text-node values redacted by the scrubber.
    -- 0 = clean session; > 0 = "session had N PII hits".
    scrubbed_count  INTEGER     NOT NULL DEFAULT 0,
    -- Total post-scrub, post-gzip byte count. Lets the
    -- dashboard estimate storage without K3 round-trip.
    byte_count      INTEGER     NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Hot path: "list every replay session for one event".
CREATE INDEX IF NOT EXISTS replay_sessions_event_idx
    ON replay_sessions (event_id, created_at DESC);

-- Hot path: "list recent replays for project P" (operator
-- triage scrolling).
CREATE INDEX IF NOT EXISTS replay_sessions_project_created_idx
    ON replay_sessions (project_id, created_at DESC);

-- Janitor: enumerate hashes for orphan-blob GC.
CREATE INDEX IF NOT EXISTS replay_sessions_blob_hash_idx
    ON replay_sessions (blob_hash);
ALTER TABLE replay_sessions ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS replay_sessions_isolation ON replay_sessions;
CREATE POLICY replay_sessions_isolation ON replay_sessions
    USING (workspace_id = current_workspace_id())
    WITH CHECK (workspace_id = current_workspace_id());
