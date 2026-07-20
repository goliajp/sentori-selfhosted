-- Sentori core migration 0003 — event-pipeline (K4).
--
-- Owns two tables: issues + events.
--
-- issues: one row per (project_id, fingerprint). UPSERT pattern
--   from the ingest pipeline handles regression detection
--   atomically: a single SQL ON CONFLICT path flips `status =
--   'resolved'` → `'regressed'` and stamps `regressed_at` /
--   `regressed_in_release` from the incoming event, with no
--   read-then-write window where the dashboard could see stale
--   `resolved` after a regression event landed.
--
-- events: append-only, per-project history. Slim typed columns
--   for the fields the dashboard facets on (kind / platform /
--   release / environment / timestamp) plus a JSONB `payload`
--   for everything else (device / app / breadcrumbs / tags /
--   user / geo / attachments / flags / bundle / framework /
--   link_hashes / symbolication / …). Per user decision
--   2026-06-20, SDK additions are zero-migration.
--
-- Partitioning: events is a SINGLE TABLE in v0.1. K9 runtime-
-- metrics will ship the partition lifecycle first; K4 will
-- reuse the pattern when retention bites. Per user decision
-- 2026-06-20.
--
-- ── multi-tenancy ─────────────────────────────────────────────
-- workspace_id NOT NULL on both, denormalized from
-- projects.workspace_id at INSERT time (the ingest pipeline
-- writes it). RLS enforces cross-workspace isolation. The
-- existing (project_id, ...) composite indexes are kept as-is;
-- RLS policy injection `AND workspace_id = X` still picks them
-- because Postgres planner evaluates the equality against the
-- STABLE current_workspace_id().

CREATE TABLE IF NOT EXISTS issues (
    id                      UUID PRIMARY KEY,
    workspace_id            UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id              UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    fingerprint             TEXT NOT NULL,
    error_type              TEXT NOT NULL,
    message_sample          TEXT NOT NULL DEFAULT '',
    kind                    TEXT NOT NULL CHECK (kind IN ('error', 'anr', 'near_crash', 'message')),
    status                  TEXT NOT NULL DEFAULT 'active'
                                CHECK (status IN ('active', 'resolved', 'regressed', 'ignored')),
    first_seen              TIMESTAMPTZ NOT NULL,
    last_seen               TIMESTAMPTZ NOT NULL,
    event_count             BIGINT NOT NULL DEFAULT 0,
    last_environment        TEXT NOT NULL,
    last_release            TEXT NOT NULL,
    regressed_at            TIMESTAMPTZ,
    regressed_in_release    TEXT,
    resolved_at             TIMESTAMPTZ,
    UNIQUE (project_id, fingerprint)
);
CREATE INDEX IF NOT EXISTS issues_project_last_seen_idx
    ON issues (project_id, last_seen DESC);
CREATE INDEX IF NOT EXISTS issues_project_status_idx
    ON issues (project_id, status);
ALTER TABLE issues ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS issues_isolation ON issues;
CREATE POLICY issues_isolation ON issues
    USING (workspace_id = current_workspace_id())
    WITH CHECK (workspace_id = current_workspace_id());

CREATE TABLE IF NOT EXISTS events (
    id              UUID PRIMARY KEY,
    workspace_id    UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id      UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    issue_id        UUID NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    timestamp       TIMESTAMPTZ NOT NULL,
    kind            TEXT NOT NULL
                        CHECK (kind IN ('error', 'anr', 'near_crash', 'message')),
    platform        TEXT NOT NULL
                        CHECK (platform IN ('javascript', 'ios', 'android')),
    release         TEXT NOT NULL,
    environment     TEXT NOT NULL,
    payload         JSONB NOT NULL DEFAULT '{}'::jsonb,
    received_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- Hot path: dashboard "show recent events for project P in time
-- range R". Composite (project_id, timestamp DESC).
CREATE INDEX IF NOT EXISTS events_project_timestamp_idx
    ON events (project_id, timestamp DESC);
-- Hot path: issue detail page "show events grouped under this
-- issue".
CREATE INDEX IF NOT EXISTS events_issue_timestamp_idx
    ON events (issue_id, timestamp DESC);
-- Append-only + time-correlated: BRIN is ~1000× smaller than
-- B-tree for this shape with no measurable scan cost penalty.
CREATE INDEX IF NOT EXISTS events_timestamp_brin_idx
    ON events USING BRIN (timestamp);
ALTER TABLE events ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS events_isolation ON events;
CREATE POLICY events_isolation ON events
    USING (workspace_id = current_workspace_id())
    WITH CHECK (workspace_id = current_workspace_id());
