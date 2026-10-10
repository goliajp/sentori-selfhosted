-- A project-scoped, signed adapter for CI/repository/deployment systems.
CREATE TABLE task_integrations (
    project_id UUID PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
    repository TEXT NOT NULL,
    environment TEXT NOT NULL,
    secret_blob BYTEA NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT true,
    version BIGINT NOT NULL DEFAULT 1,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE task_signal_deliveries (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    delivery_id TEXT NOT NULL,
    configuration_version BIGINT NOT NULL,
    provider TEXT NOT NULL DEFAULT 'signed-webhook',
    repository TEXT NOT NULL,
    commit_sha TEXT NOT NULL,
    environment TEXT,
    kind TEXT NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL,
    received_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    signature_verified BOOLEAN NOT NULL CHECK (signature_verified),
    digest TEXT NOT NULL,
    payload JSONB NOT NULL,
    outcome TEXT NOT NULL,
    UNIQUE(project_id, delivery_id)
);
CREATE INDEX task_signal_evidence_idx ON task_signal_deliveries(task_id,configuration_version,occurred_at DESC,id DESC);
