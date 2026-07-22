-- Sentori core migration 0011 — integrations (K12).
--
-- Two tables:
--   integrations — per-project adapter config (OAuth tokens,
--     webhook URLs, etc.). UNIQUE (project_id, kind) — one
--     config per (project, integration kind).
--   issue_integration_links — back-link from Sentori issue
--     to upstream item id/url. UNIQUE (issue_id, kind) so
--     re-creating dispatches don't dup.
--
-- Note: Sentori v0.1 K1 ships a single-workspace identity
-- model (no `organizations` table) — `projects` is the
-- multi-tenant unit, so integrations attach per project
-- rather than per org. Legacy had `org_id` because legacy
-- server had organizations.
--
-- Per K12 design (autonomous, 2026-06-21):
--   - K12 = trait + service + Slack reference adapter.
--   - K12.1-K12.4 = Linear / Jira / GitHub / GitLab vendor
--     impls (OAuth flow per vendor, defer).
--   - `kind` is TEXT (not enum) so any adapter can register
--     without altering schema.

-- ── multi-tenancy ──────────────────────────────────────────
-- workspace_id NOT NULL on both, denormalized at INSERT. RLS
-- enforces isolation. The reverse-lookup index on
-- issue_integration_links is widened to include workspace_id
-- so a webhook from upstream can't accidentally resolve to
-- the wrong tenant's issue.

CREATE TABLE IF NOT EXISTS integrations (
    id            UUID         PRIMARY KEY,
    workspace_id  UUID         NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id    UUID         NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    -- 'slack' | 'linear' | 'jira' | 'github' | 'gitlab' |
    -- whatever future adapters add via `kind()`. TEXT not
    -- enum so adding a vendor adapter is a code-only change.
    kind          TEXT         NOT NULL,
    -- Vendor-specific blob. Returned by `exchange_code` for
    -- OAuth adapters or `accept_manual_config` for manual
    -- ones. Read back during dispatch + status updates.
    config        JSONB        NOT NULL DEFAULT '{}'::jsonb,
    -- Who clicked Connect.
    connected_by  UUID         REFERENCES users(id) ON DELETE SET NULL,
    connected_at  TIMESTAMPTZ  NOT NULL DEFAULT now(),
    -- Operator can deactivate without losing the OAuth
    -- token; flipping back to active resumes dispatch.
    active        BOOLEAN      NOT NULL DEFAULT TRUE
);
-- One config per (project, kind).
CREATE UNIQUE INDEX IF NOT EXISTS integrations_project_kind_idx
    ON integrations (project_id, kind);
-- "Show me everything for project X" — small set, partial
-- index keeps the active-rows scan O(1).
CREATE INDEX IF NOT EXISTS integrations_project_active_idx
    ON integrations (project_id) WHERE active = TRUE;
ALTER TABLE integrations ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS integrations_isolation ON integrations;
CREATE POLICY integrations_isolation ON integrations
    USING (workspace_id = current_workspace_id())
    WITH CHECK (workspace_id = current_workspace_id());

CREATE TABLE IF NOT EXISTS issue_integration_links (
    id            UUID         PRIMARY KEY,
    workspace_id  UUID         NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    -- K5 issue this link belongs to.
    issue_id      UUID         NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    kind          TEXT         NOT NULL,
    -- Upstream item identifier (Linear issue id, GitHub
    -- issue number, Slack message ts, …). String so we
    -- don't constrain the vendor's id shape.
    external_id   TEXT         NOT NULL,
    -- Browser-clickable URL.
    external_url  TEXT         NOT NULL,
    -- For audit + retry decisions.
    created_at    TIMESTAMPTZ  NOT NULL DEFAULT now()
);
-- One link per (issue, kind) — re-dispatch on the same kind
-- is a no-op.
CREATE UNIQUE INDEX IF NOT EXISTS issue_integration_links_issue_kind_idx
    ON issue_integration_links (issue_id, kind);
-- "Which Sentori issue maps to this upstream id?" reverse
-- lookup — webhook ingest from upstream uses this. Widened to
-- include workspace_id so a webhook scoped to tenant X can't
-- resolve to tenant Y's matching external_id.
CREATE INDEX IF NOT EXISTS issue_integration_links_kind_external_idx
    ON issue_integration_links (workspace_id, kind, external_id);
ALTER TABLE issue_integration_links ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS issue_integration_links_isolation ON issue_integration_links;
CREATE POLICY issue_integration_links_isolation ON issue_integration_links
    USING (workspace_id = current_workspace_id())
    WITH CHECK (workspace_id = current_workspace_id());
