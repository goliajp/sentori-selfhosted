-- Sentori core migration 0018 — release health sessions.
--
-- One row per SDK session ping (`/v1/sessions`). The SDK starts
-- a session when the app comes to foreground; status transitions
-- to crashed/errored on the first captureException, exited on
-- backgrounding. Sessions are the input to the crash-free-rate
-- visualisation.
--
-- Columns mirror legacy `server/migrations/0021_sessions.sql`
-- verbatim — `user_id` is the app-side user TEXT, NOT an FK to
-- the sentori `users` table.

CREATE TABLE IF NOT EXISTS sessions (
    id           UUID        PRIMARY KEY,
    workspace_id UUID        NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id   UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    user_id      TEXT,
    release      TEXT        NOT NULL,
    environment  TEXT        NOT NULL,
    status       TEXT        NOT NULL CHECK (status IN ('ok', 'errored', 'crashed', 'exited')),
    started_at   TIMESTAMPTZ NOT NULL,
    duration_ms  INTEGER     NOT NULL DEFAULT 0 CHECK (duration_ms >= 0),
    received_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS sessions_project_release_idx
    ON sessions (project_id, release, received_at DESC);
CREATE INDEX IF NOT EXISTS sessions_project_received_idx
    ON sessions (project_id, received_at DESC);
CREATE INDEX IF NOT EXISTS sessions_project_user_idx
    ON sessions (project_id, user_id)
    WHERE user_id IS NOT NULL;
