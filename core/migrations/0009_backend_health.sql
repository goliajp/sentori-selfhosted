-- Backend availability checks (design: the SDK carries the URL the
-- integrator wrote in init(); the server probes it and keeps a
-- rolling day of results).
ALTER TABLE projects
    ADD COLUMN backend_health_url text;

CREATE TABLE backend_checks (
    project_id uuid NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    checked_at timestamptz NOT NULL DEFAULT now(),
    ok boolean NOT NULL,
    status_code integer,
    latency_ms integer,
    PRIMARY KEY (project_id, checked_at)
);
