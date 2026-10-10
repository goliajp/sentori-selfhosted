CREATE TABLE monitors (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
    url TEXT NOT NULL CHECK (length(url) <= 2048),
    interval_seconds INTEGER NOT NULL CHECK (interval_seconds BETWEEN 60 AND 86400),
    timeout_seconds INTEGER NOT NULL CHECK (timeout_seconds BETWEEN 1 AND 30),
    expected_status INTEGER NOT NULL CHECK ((expected_status BETWEEN 200 AND 299 OR expected_status BETWEEN 400 AND 599)),
    failure_threshold INTEGER NOT NULL CHECK (failure_threshold BETWEEN 1 AND 10),
    enabled BOOLEAN NOT NULL,
    version BIGINT NOT NULL DEFAULT 1,
    generation BIGINT NOT NULL,
    next_check_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    lease_owner UUID,
    lease_until TIMESTAMPTZ,
    status TEXT NOT NULL DEFAULT 'unknown' CHECK (status IN ('unknown','operational','degraded','down')),
    consecutive_failures INTEGER NOT NULL DEFAULT 0,
    incident_id UUID,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK ((lease_owner IS NULL) = (lease_until IS NULL))
);
CREATE INDEX monitors_due_idx ON monitors(next_check_at) WHERE enabled AND deleted_at IS NULL;
CREATE INDEX monitors_project_idx ON monitors(project_id,created_at,id);
CREATE TABLE monitor_samples (
    id UUID PRIMARY KEY,
    monitor_id UUID NOT NULL REFERENCES monitors(id) ON DELETE CASCADE,
    config_version BIGINT NOT NULL,
    checked_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    ok BOOLEAN NOT NULL,
    status_code INTEGER,
    latency_ms INTEGER NOT NULL,
    error_code TEXT
);
CREATE INDEX monitor_samples_page_idx ON monitor_samples(monitor_id,checked_at DESC,id DESC);
CREATE INDEX monitor_samples_retention_idx ON monitor_samples(checked_at);
CREATE TABLE monitor_events (
    id UUID PRIMARY KEY,
    monitor_id UUID NOT NULL REFERENCES monitors(id) ON DELETE CASCADE,
    incident_id UUID NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('monitor.down','monitor.recovered')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(incident_id,kind)
);
CREATE TABLE monitor_commands (
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    actor_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    command_id UUID NOT NULL,
    digest TEXT NOT NULL,
    response JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY(project_id,actor_user_id,command_id)
);
ALTER TABLE inbox_items ALTER COLUMN task_id DROP NOT NULL;
ALTER TABLE inbox_items ALTER COLUMN source_event_id DROP NOT NULL;
ALTER TABLE inbox_items ADD COLUMN monitor_id UUID REFERENCES monitors(id) ON DELETE CASCADE;
ALTER TABLE inbox_items ADD COLUMN monitor_event_id UUID REFERENCES monitor_events(id) ON DELETE CASCADE;
ALTER TABLE inbox_items DROP CONSTRAINT inbox_items_category_check;
ALTER TABLE inbox_items ADD CONSTRAINT inbox_items_category_check CHECK (category IN ('assignment','mention','task','monitor'));
ALTER TABLE inbox_items ADD CONSTRAINT inbox_items_source_check CHECK (
    (category='monitor' AND monitor_id IS NOT NULL AND monitor_event_id IS NOT NULL AND task_id IS NULL AND source_event_id IS NULL) OR
    (category<>'monitor' AND monitor_id IS NULL AND monitor_event_id IS NULL AND task_id IS NOT NULL AND source_event_id IS NOT NULL)
);
CREATE UNIQUE INDEX inbox_monitor_event_idx ON inbox_items(user_id,monitor_event_id);
