-- Sentori core migration 0014 — saved views (K15).
--
-- A "view" is a named snapshot of a filterable list: target
-- + name + payload (query JSON the dashboard rebuilds the UI
-- state from). One row per saved view.
--
-- Two scopes (v0.1 simplification of legacy's 3):
--   - personal: visible only to the creating user.
--   - workspace: visible to every workspace member.
--
-- Legacy had a third "team" scope keyed on a teams table.
-- Sentori v0.1 K1 has no teams table → team scope dropped;
-- adding it later is a code-only change to the enum +
-- migration to add the `team_id` column.
--
-- Targets shipped from day one (matching K-tier business
-- surfaces): issues / events / spans / replays / metrics.
-- The CHECK keeps the enum tight while leaving the payload
-- vendor-agnostic.
--
-- project_id is a top-level column (not buried in payload)
-- so per-project view lookups can be index-served. NULL =
-- workspace-wide view (operator-defined preset showing
-- across every project).
--
-- Per K15 design (autonomous, 2026-06-21):
--   - No org_id column — v0.1 K1 is single-workspace.
--   - Scope ↔ FK polarity enforced via CHECK so app code
--     doesn't need to relitigate the rule on every write.

-- ── multi-tenancy ──────────────────────────────────────────
-- workspace_id NOT NULL even when project_id IS NULL
-- (workspace-wide view still belongs to a workspace). RLS
-- enforces isolation. The workspace-wide partial index widens
-- to (workspace_id, target) so each workspace's preset list
-- is index-served.

CREATE TABLE IF NOT EXISTS saved_views (
    id            UUID         PRIMARY KEY,
    workspace_id  UUID         NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id    UUID         REFERENCES projects(id) ON DELETE CASCADE,
    target      TEXT         NOT NULL CHECK (target IN
        ('issues', 'events', 'spans', 'replays', 'metrics')),
    scope       TEXT         NOT NULL CHECK (scope IN ('personal', 'workspace')),
    user_id     UUID         REFERENCES users(id) ON DELETE CASCADE,
    name        TEXT         NOT NULL,
    -- Payload is the saved query state — dashboard rebuilds
    -- the filter UI from it. K15 doesn't validate shape.
    payload     JSONB        NOT NULL DEFAULT '{}'::jsonb,
    created_at  TIMESTAMPTZ  NOT NULL DEFAULT now(),
    created_by  UUID         REFERENCES users(id) ON DELETE SET NULL,
    updated_at  TIMESTAMPTZ  NOT NULL DEFAULT now(),

    -- Scope ↔ FK polarity:
    --   personal → user_id NOT NULL
    --   workspace → user_id IS NULL (everyone sees)
    CHECK (
        (scope = 'personal'  AND user_id IS NOT NULL)
     OR (scope = 'workspace' AND user_id IS NULL)
    )
);

-- "What views are saved for project P, target T?" — main
-- dashboard query.
CREATE INDEX IF NOT EXISTS saved_views_project_target_idx
    ON saved_views (project_id, target)
    WHERE project_id IS NOT NULL;

-- "Workspace-wide views for target T" — operator preset
-- chooser. Widened from (target) to (workspace_id, target).
CREATE INDEX IF NOT EXISTS saved_views_workspace_target_idx
    ON saved_views (workspace_id, target)
    WHERE project_id IS NULL;

-- "My personal views, any target" — user profile drawer.
CREATE INDEX IF NOT EXISTS saved_views_user_idx
    ON saved_views (user_id, target)
    WHERE user_id IS NOT NULL;

ALTER TABLE saved_views ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS saved_views_isolation ON saved_views;
CREATE POLICY saved_views_isolation ON saved_views
    USING (workspace_id = current_workspace_id())
    WITH CHECK (workspace_id = current_workspace_id());
