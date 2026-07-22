-- Sentori core migration 0027 — workspace-level dashboard catalogs
-- (workspace_labels (a.k.a. org_labels) + workspace_quotas + usage_counters
--  + integration_templates).
--
-- Legacy `org_labels` → v0.2 `workspace_labels`; the workspace
-- IS the org from a data-portability standpoint. Column names
-- otherwise verbatim.
--
-- Source migrations:
--   workspace_labels        — 0061_org_labels.sql
--   workspace_quotas + usage_counters — 0009_quotas.sql
--   integration_templates   — 0063_integration_templates.sql
--
-- Note: v0.1 0001 already constrains projects via a separate
-- privacy-salt model; the legacy
-- `ALTER TABLE projects ADD COLUMN fingerprint_with_labels` is
-- omitted because labels apply at the workspace level in v0.2.

CREATE TABLE IF NOT EXISTS workspace_labels (
    id                 UUID        PRIMARY KEY,
    workspace_id       UUID        NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name               TEXT        NOT NULL,
    color              TEXT,
    sla_priority_hours INTEGER,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, name)
);

CREATE INDEX IF NOT EXISTS workspace_labels_workspace_idx
    ON workspace_labels (workspace_id);

CREATE TABLE IF NOT EXISTS workspace_quotas (
    workspace_id        UUID        PRIMARY KEY REFERENCES workspaces(id) ON DELETE CASCADE,
    plan                TEXT        NOT NULL DEFAULT 'free',
    event_limit_monthly INTEGER     NOT NULL,
    retention_days      INTEGER     NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS usage_counters (
    workspace_id  UUID        NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    period_yyyymm TEXT        NOT NULL,
    event_count   BIGINT      NOT NULL DEFAULT 0,
    dropped_count BIGINT      NOT NULL DEFAULT 0,
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, period_yyyymm)
);

CREATE INDEX IF NOT EXISTS usage_counters_period_idx
    ON usage_counters (period_yyyymm);

CREATE TABLE IF NOT EXISTS integration_templates (
    id                       UUID        PRIMARY KEY,
    owner_user_id            UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind                     TEXT        NOT NULL,
    name                     TEXT        NOT NULL,
    config                   JSONB       NOT NULL DEFAULT '{}'::jsonb,
    shared_with_workspace_id UUID        REFERENCES workspaces(id) ON DELETE SET NULL,
    created_at               TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at               TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS integration_templates_owner_idx
    ON integration_templates (owner_user_id);
CREATE INDEX IF NOT EXISTS integration_templates_shared_workspace_idx
    ON integration_templates (shared_with_workspace_id)
    WHERE shared_with_workspace_id IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS integration_templates_owner_kind_name_uniq
    ON integration_templates (owner_user_id, kind, name);
