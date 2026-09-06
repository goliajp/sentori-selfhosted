-- Sentori v1 migration 0002 — projects, assignments, tokens.
--
-- project_assignments is the whole of the permission model: the
-- superadmin sees everything without a row here; an admin sees
-- exactly the projects they were assigned (design.md §9). One flat
-- table instead of the v0.2 workspace RBAC + per-project visibility
-- + invite machinery.
--
-- tokens carry a scope, not a kind:
--   ingest — held by the SDK inside a shipped app; can only write
--            events and attachments.
--   api    — held by automation / AI agents; can read issues, pull
--            bundles, change status, append notes. This is the key
--            the AI closed loop runs on (design.md §9).
-- Multiple named tokens per project so rotation is create-new →
-- switch clients → revoke-old, never a hard cutover that strands
-- shipped apps (the zero-cost rule reaches key management too).

CREATE TABLE projects (
    id         UUID        PRIMARY KEY,
    name       TEXT        NOT NULL,
    platform   TEXT        NOT NULL DEFAULT 'react-native',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE project_assignments (
    user_id     UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    project_id  UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    assigned_by UUID        REFERENCES users(id) ON DELETE SET NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, project_id)
);
CREATE INDEX project_assignments_project_idx
    ON project_assignments (project_id);

CREATE TABLE tokens (
    id           UUID        PRIMARY KEY,
    project_id   UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name         TEXT        NOT NULL,
    scope        TEXT        NOT NULL CHECK (scope IN ('ingest', 'api')),
    token_hash   TEXT        NOT NULL UNIQUE,
    last4        TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_used_at TIMESTAMPTZ,
    revoked_at   TIMESTAMPTZ
);
CREATE INDEX tokens_project_active_idx
    ON tokens (project_id) WHERE revoked_at IS NULL;
