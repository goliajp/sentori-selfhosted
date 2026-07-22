-- Sentori core migration 0021 — user_reports (feedback).
--
-- `/v1/user-reports` payloads: end-user-typed feedback attached
-- to a crashed event. The dashboard renders the title/body in
-- the issue detail "User reports" tab so triagers can read what
-- the affected user wrote without leaving the issue.
--
-- Columns verbatim from legacy
-- `server/migrations/0036_user_reports.sql`.

CREATE TABLE IF NOT EXISTS user_reports (
    id           UUID        PRIMARY KEY,
    workspace_id UUID        NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id   UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    event_id     UUID,
    issue_id     UUID        REFERENCES issues(id) ON DELETE SET NULL,
    title        TEXT        NOT NULL CHECK (char_length(title) BETWEEN 1 AND 200),
    body         TEXT        NOT NULL CHECK (char_length(body) BETWEEN 1 AND 8000),
    email        TEXT        CHECK (email IS NULL OR char_length(email) <= 320),
    name         TEXT        CHECK (name IS NULL OR char_length(name) <= 200),
    received_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS user_reports_project_received_idx
    ON user_reports (project_id, received_at DESC);
CREATE INDEX IF NOT EXISTS user_reports_issue_idx
    ON user_reports (issue_id)
    WHERE issue_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS user_reports_event_idx
    ON user_reports (event_id)
    WHERE event_id IS NOT NULL;
