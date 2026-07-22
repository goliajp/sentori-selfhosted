-- Sentori core migration 0030 — culprit_commits + pii_findings + pii_scan_cursor.
--
-- `culprit_commits` records the git commit suspected of
-- introducing a given issue. Source is 'manual' when a user
-- pins the commit from the dashboard, 'auto' when the GitHub /
-- GitLab integration's blame heuristic proposes one.
--
-- `pii_findings` is the scanner output for the per-release
-- privacy audit; `pii_scan_cursor` is the per-event idempotency
-- key so re-runs skip already-scanned rows.
--
-- Columns verbatim from legacy
-- `0042_culprit_commits.sql` + `0041_pii_findings.sql`.

CREATE TABLE IF NOT EXISTS culprit_commits (
    id           UUID        PRIMARY KEY,
    workspace_id UUID        NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id   UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    issue_id     UUID        NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    commit_sha   TEXT        NOT NULL CHECK (char_length(commit_sha) BETWEEN 7 AND 64),
    author       TEXT,
    message      TEXT,
    committed_at TIMESTAMPTZ,
    html_url     TEXT,
    confidence   INTEGER     NOT NULL DEFAULT 100 CHECK (confidence BETWEEN 0 AND 100),
    source       TEXT        NOT NULL DEFAULT 'manual' CHECK (source IN ('manual', 'auto')),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (issue_id, commit_sha)
);

CREATE INDEX IF NOT EXISTS culprit_commits_issue_created_idx
    ON culprit_commits (issue_id, created_at DESC);

CREATE TABLE IF NOT EXISTS pii_findings (
    id           UUID        PRIMARY KEY,
    workspace_id UUID        NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id   UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    release      TEXT        NOT NULL,
    event_id     UUID        NOT NULL,
    field_path   TEXT        NOT NULL,
    pattern_kind TEXT        NOT NULL CHECK (pattern_kind IN ('email', 'phone', 'cc-like', 'address-like')),
    sample       TEXT        NOT NULL,
    seen_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS pii_findings_project_release_idx
    ON pii_findings (project_id, release, seen_at DESC);
CREATE INDEX IF NOT EXISTS pii_findings_event_idx
    ON pii_findings (event_id);

CREATE TABLE IF NOT EXISTS pii_scan_cursor (
    event_id   UUID        PRIMARY KEY,
    scanned_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS pii_scan_cursor_scanned_at_idx
    ON pii_scan_cursor (scanned_at);
