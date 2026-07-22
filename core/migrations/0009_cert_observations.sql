-- Sentori core migration 0009 — cert-monitor (K10).
--
-- Two tables for Certificate Transparency monitoring via
-- crt.sh poll:
--
--   cert_watch_domains — which (project, domain) pairs the
--     operator has subscribed to. Polling fan-out reads this
--     table once per tick.
--
--   cert_observations — every distinct cert ever seen issued
--     for a watched domain. UNIQUE (project_id, cert_id)
--     drops re-poll dupes; an `ON CONFLICT DO NOTHING ...
--     RETURNING id` that returns a row means the cert is
--     genuinely new and worth notifying about.
--
-- Both tables FK to projects(id) ON DELETE CASCADE so
-- project deletion cleans up the watch + observation
-- history. No partitioning — cert observation count per
-- project is bounded by the watched-domain count × CT log
-- issuance rate (typically < 1000 / domain / year), well
-- under a partition's worth.

-- ── multi-tenancy ──────────────────────────────────────────
-- workspace_id NOT NULL on both, denormalized from
-- projects.workspace_id at INSERT. RLS enforces isolation.

CREATE TABLE IF NOT EXISTS cert_watch_domains (
    id           UUID         PRIMARY KEY,
    workspace_id UUID         NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id   UUID         NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    -- Lowercase apex domain (e.g. `example.com`). The crt.sh
    -- query wraps with `%.` to wildcard subdomains.
    domain      TEXT         NOT NULL,
    added_by    UUID         REFERENCES users(id) ON DELETE SET NULL,
    added_at    TIMESTAMPTZ  NOT NULL DEFAULT now(),
    -- Last successful poll completion (NULL means never).
    last_polled_at TIMESTAMPTZ
);
-- One watch per (project, domain).
CREATE UNIQUE INDEX IF NOT EXISTS cert_watch_domains_project_domain_idx
    ON cert_watch_domains (project_id, domain);
ALTER TABLE cert_watch_domains ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS cert_watch_domains_isolation ON cert_watch_domains;
CREATE POLICY cert_watch_domains_isolation ON cert_watch_domains
    USING (workspace_id = current_workspace_id())
    WITH CHECK (workspace_id = current_workspace_id());

CREATE TABLE IF NOT EXISTS cert_observations (
    id            UUID         PRIMARY KEY,
    workspace_id  UUID         NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id    UUID         NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    -- The watched domain that surfaced this cert (might be a
    -- subdomain match via crt.sh's `%.` wildcard).
    domain        TEXT         NOT NULL,
    -- crt.sh's own integer cert id. Stable across crt.sh
    -- responses for the same cert; we dedup on it.
    cert_id       BIGINT       NOT NULL,
    common_name   TEXT,
    -- Comma-separated SANs. crt.sh sometimes returns large
    -- name_value blobs (~8 KB); the K10 ingest truncates.
    name_value    TEXT,
    issuer_name   TEXT         NOT NULL,
    not_before    TIMESTAMPTZ  NOT NULL,
    not_after     TIMESTAMPTZ  NOT NULL,
    observed_at   TIMESTAMPTZ  NOT NULL DEFAULT now()
);
-- Dedup key: a re-poll of the same cert for the same
-- project is a no-op via ON CONFLICT.
CREATE UNIQUE INDEX IF NOT EXISTS cert_observations_project_cert_idx
    ON cert_observations (project_id, cert_id);
-- "What's about to expire on this project?" — fast scan.
CREATE INDEX IF NOT EXISTS cert_observations_project_not_after_idx
    ON cert_observations (project_id, not_after);
-- "Recent activity for project P" — operator inbox.
CREATE INDEX IF NOT EXISTS cert_observations_project_observed_idx
    ON cert_observations (project_id, observed_at DESC);
ALTER TABLE cert_observations ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS cert_observations_isolation ON cert_observations;
CREATE POLICY cert_observations_isolation ON cert_observations
    USING (workspace_id = current_workspace_id())
    WITH CHECK (workspace_id = current_workspace_id());
