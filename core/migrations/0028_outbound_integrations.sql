-- Credentials (including webhook URL paths) are sealed with the operator-held master key.
ALTER TABLE projects ADD COLUMN webhook_secret_blob BYTEA;
CREATE INDEX projects_legacy_webhook_idx ON projects(id) WHERE webhook_url IS NOT NULL;
CREATE TABLE outbound_integrations (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('email','webhook','github','jira')),
    enabled BOOLEAN NOT NULL,
    events TEXT[] NOT NULL,
    summary JSONB NOT NULL,
    secret_blob BYTEA NOT NULL,
    version BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(project_id,kind)
);
CREATE TABLE integration_commands (
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    actor_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    command_id UUID NOT NULL,
    digest TEXT NOT NULL,
    response JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY(project_id,actor_user_id,command_id)
);
CREATE TABLE notification_events (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    source_key TEXT NOT NULL,
    kind TEXT NOT NULL,
    payload JSONB NOT NULL,
    issue_id UUID REFERENCES issues(id) ON DELETE CASCADE,
    actor_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    target_integration_id UUID REFERENCES outbound_integrations(id) ON DELETE CASCADE,
    generation BIGINT NOT NULL,
    routed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(project_id,source_key)
);
CREATE INDEX notification_events_pending_idx ON notification_events(created_at,id) WHERE routed_at IS NULL;
CREATE TABLE notification_deliveries (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    event_id UUID NOT NULL REFERENCES notification_events(id) ON DELETE CASCADE,
    integration_id UUID REFERENCES outbound_integrations(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    destination TEXT NOT NULL CHECK (destination IN ('integration','user_email','legacy_webhook')),
    config_version BIGINT NOT NULL,
    state TEXT NOT NULL DEFAULT 'queued' CHECK (state IN ('queued','running','retry_wait','delivered','failed','uncertain','cancelled')),
    attempt INTEGER NOT NULL DEFAULT 0 CHECK (attempt BETWEEN 0 AND 5),
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    lease_owner UUID,
    lease_until TIMESTAMPTZ,
    error_code TEXT,
    http_status INTEGER,
    external_url TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    finished_at TIMESTAMPTZ,
    CHECK ((state='running') = (lease_owner IS NOT NULL AND lease_until IS NOT NULL)),
    CHECK ((destination='integration' AND integration_id IS NOT NULL AND user_id IS NULL) OR
           (destination='user_email' AND user_id IS NOT NULL AND integration_id IS NULL) OR
           (destination='legacy_webhook' AND user_id IS NULL AND integration_id IS NULL))
);
CREATE UNIQUE INDEX notification_delivery_dedup_idx ON notification_deliveries(event_id,integration_id,user_id,destination) NULLS NOT DISTINCT;
CREATE INDEX notification_delivery_due_idx ON notification_deliveries(next_attempt_at,id) WHERE state IN ('queued','retry_wait','running');
CREATE INDEX notification_delivery_project_idx ON notification_deliveries(project_id,created_at DESC,id DESC);
