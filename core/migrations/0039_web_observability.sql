-- RUM and laboratory checks deliberately have separate storage and denominators.
CREATE TABLE web_visits (
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    app_id UUID NOT NULL,
    id UUID NOT NULL,
    revision INTEGER NOT NULL CHECK(revision>0),
    started_at TIMESTAMPTZ NOT NULL,
    received_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    release TEXT NOT NULL,
    environment TEXT NOT NULL,
    route TEXT NOT NULL,
    device_class TEXT NOT NULL CHECK(device_class IN ('mobile','desktop')),
    navigation_type TEXT NOT NULL CHECK(navigation_type IN ('document','bfcache','spa')),
    sample_rate DOUBLE PRECISION NOT NULL CHECK(sample_rate>0 AND sample_rate<=1),
    browser TEXT NOT NULL,
    error_count INTEGER NOT NULL CHECK(error_count BETWEEN 0 AND 1000),
    resource_error_count INTEGER NOT NULL CHECK(resource_error_count BETWEEN 0 AND 1000),
    metrics JSONB NOT NULL,
    missing JSONB NOT NULL,
    diagnostics JSONB NOT NULL,
    PRIMARY KEY(project_id,id),
    FOREIGN KEY(app_id,project_id) REFERENCES apps(id,project_id) ON DELETE CASCADE
);
CREATE INDEX web_visits_app_time ON web_visits(project_id,app_id,started_at DESC);
CREATE INDEX web_visits_retention ON web_visits(received_at);

CREATE TABLE web_checks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    app_id UUID NOT NULL,
    configuration JSONB NOT NULL,
    version BIGINT NOT NULL DEFAULT 1,
    enabled BOOLEAN NOT NULL DEFAULT false,
    interval_seconds INTEGER NOT NULL DEFAULT 0 CHECK(interval_seconds=0 OR interval_seconds BETWEEN 900 AND 604800),
    consecutive_failures INTEGER NOT NULL DEFAULT 0,
    incident_id UUID,
    health TEXT NOT NULL DEFAULT 'unknown' CHECK(health IN ('unknown','healthy','degraded','failing','monitor-error')),
    next_run_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY(app_id,project_id) REFERENCES apps(id,project_id) ON DELETE CASCADE,
    UNIQUE(id,project_id)
);
CREATE TABLE web_check_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    check_id UUID NOT NULL,
    app_id UUID NOT NULL,
    config_version BIGINT NOT NULL,
    configuration JSONB NOT NULL,
    generation BIGINT NOT NULL,
    status TEXT NOT NULL DEFAULT 'queued' CHECK(status IN ('queued','running','passed','failed','error','cancelled')),
    trigger TEXT NOT NULL CHECK(trigger IN ('manual','deployment','schedule')),
    lease_id UUID,
    lease_until TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    result JSONB,
    FOREIGN KEY(check_id,project_id) REFERENCES web_checks(id,project_id) ON DELETE CASCADE,
    FOREIGN KEY(app_id,project_id) REFERENCES apps(id,project_id) ON DELETE CASCADE
);
CREATE INDEX web_check_runs_scope ON web_check_runs(project_id,app_id,created_at DESC);
CREATE INDEX web_check_runs_pending ON web_check_runs(created_at) WHERE status IN ('queued','running');

-- Respect the same project transfer fence as other project-owned writes.
CREATE TRIGGER transfer_write_guard BEFORE INSERT OR UPDATE OR DELETE ON web_visits FOR EACH ROW EXECUTE FUNCTION sentori_transfer_write_guard();
CREATE TRIGGER transfer_write_guard BEFORE INSERT OR UPDATE OR DELETE ON web_checks FOR EACH ROW EXECUTE FUNCTION sentori_transfer_write_guard();
CREATE TRIGGER transfer_write_guard BEFORE INSERT OR UPDATE OR DELETE ON web_check_runs FOR EACH ROW EXECUTE FUNCTION sentori_transfer_write_guard();

CREATE TABLE web_check_commands (
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    id UUID NOT NULL,
    request_hash TEXT NOT NULL,
    result JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY(project_id,id)
);
CREATE TRIGGER transfer_write_guard BEFORE INSERT OR UPDATE OR DELETE ON web_check_commands FOR EACH ROW EXECUTE FUNCTION sentori_transfer_write_guard();
