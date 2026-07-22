-- Sentori core migration 0019 — track_events (analytics).
--
-- `/v1/track:batch` events. Distinct from `events` (the error /
-- log pipeline) — these are intentional product analytics with
-- arbitrary `props`. The dashboard funnel / metric views read
-- from here.
--
-- Columns verbatim from legacy
-- `server/migrations/0046_track_events.sql`. `user_id` is the
-- app-side TEXT identifier, not an FK.

CREATE TABLE IF NOT EXISTS track_events (
    id           UUID        PRIMARY KEY,
    workspace_id UUID        NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id   UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name         TEXT        NOT NULL,
    user_id      TEXT,
    session_id   UUID,
    route        TEXT,
    release      TEXT,
    environment  TEXT,
    props        JSONB       NOT NULL DEFAULT '{}'::jsonb,
    occurred_at  TIMESTAMPTZ NOT NULL,
    received_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS track_events_project_ts_idx
    ON track_events (project_id, occurred_at DESC);
CREATE INDEX IF NOT EXISTS track_events_project_name_ts_idx
    ON track_events (project_id, name, occurred_at DESC);
CREATE INDEX IF NOT EXISTS track_events_project_user_ts_idx
    ON track_events (project_id, user_id, occurred_at DESC)
    WHERE user_id IS NOT NULL;
