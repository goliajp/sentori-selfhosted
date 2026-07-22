-- Sentori core migration 0020 — security_events + user_federation_links.
--
-- `security_events` feeds the trust-score / risk dashboard.
-- The SDK posts to `/v1/security:report` whenever the host app
-- detects a notable auth or fraud signal (failed login, OTP
-- reuse, jailbreak detected, etc.). `kind` is free-form text;
-- the analytic rollup is per-kind.
--
-- `user_federation_links` records the (provider, subject) tuple
-- the SDK reports from `/v1/security/link` after a successful
-- SSO. It lets the dashboard correlate sentori's `user_id` with
-- Auth0 / Cognito / Firebase identities.
--
-- Columns verbatim from legacy
-- `server/migrations/0047_security_events.sql` +
-- `0048_user_federation_links.sql`.

CREATE TABLE IF NOT EXISTS security_events (
    id           UUID        PRIMARY KEY,
    workspace_id UUID        NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id   UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    kind         TEXT        NOT NULL,
    user_id      TEXT,
    install_id   TEXT,
    release      TEXT,
    environment  TEXT,
    country      TEXT,
    asn          INTEGER,
    asn_org      TEXT,
    server_name  TEXT,
    data         JSONB       NOT NULL DEFAULT '{}'::jsonb,
    occurred_at  TIMESTAMPTZ NOT NULL,
    received_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS security_events_project_ts_idx
    ON security_events (project_id, occurred_at DESC);
CREATE INDEX IF NOT EXISTS security_events_project_kind_ts_idx
    ON security_events (project_id, kind, occurred_at DESC);
CREATE INDEX IF NOT EXISTS security_events_project_install_ts_idx
    ON security_events (project_id, install_id, occurred_at DESC)
    WHERE install_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS security_events_project_asn_ts_idx
    ON security_events (project_id, asn, occurred_at DESC)
    WHERE asn IS NOT NULL;

CREATE TABLE IF NOT EXISTS user_federation_links (
    id           UUID        PRIMARY KEY,
    workspace_id UUID        NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id   UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    provider     TEXT        NOT NULL,
    subject      TEXT        NOT NULL,
    user_id      TEXT,
    install_id   TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, provider, subject)
);

CREATE INDEX IF NOT EXISTS user_federation_links_provider_subject_idx
    ON user_federation_links (provider, subject);
