-- Sentori core migration 0016 — SDK ingest tokens.
--
-- `tokens` is the bearer-credential row backing `st_pk_…` and
-- `st_admin_…` strings the SDK + admin CLI send on every
-- `/v1/*` request. Column names mirror legacy
-- `server/migrations/0001_init.sql` + `0008_tokens_meta.sql`
-- so the SaaS ETL can INSERT … SELECT directly without
-- per-column rename.
--
-- The token plaintext is never persisted; only its SHA-256
-- (hex-encoded TEXT in `token_hash`). `last4` keeps the trailing
-- four chars so the dashboard can show "st_pk_…abcd" without
-- ever recovering the secret.
--
-- workspace_id NOT NULL denormalised from projects.workspace_id
-- so future RLS can isolate without an extra JOIN. RLS is not
-- enabled in this migration — see infra Phase for the rollout.

CREATE TABLE IF NOT EXISTS tokens (
    id           UUID        PRIMARY KEY,
    workspace_id UUID        NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id   UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    token_hash   TEXT        NOT NULL UNIQUE,
    kind         TEXT        NOT NULL CHECK (kind IN ('public', 'admin')),
    label        TEXT,
    last4        TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    revoked_at   TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS tokens_project_id_idx
    ON tokens (project_id);
CREATE INDEX IF NOT EXISTS tokens_workspace_active_idx
    ON tokens (workspace_id)
    WHERE revoked_at IS NULL;
