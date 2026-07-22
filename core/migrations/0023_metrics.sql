-- Sentori core migration 0023 — custom metrics timeseries.
--
-- `/v1/metrics:batch` payloads — caller-defined timeseries
-- (`name`, `value`, `tags`, `ts`). Feeds the metrics-explorer
-- view in the dashboard. Distinct from runtime_metrics (0008)
-- which is sentori-defined RN runtime telemetry.
--
-- Columns verbatim from legacy
-- `server/migrations/0037_metrics.sql`.

CREATE TABLE IF NOT EXISTS metrics (
    id           UUID             PRIMARY KEY,
    workspace_id UUID             NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id   UUID             NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name         TEXT             NOT NULL CHECK (char_length(name) BETWEEN 1 AND 200),
    value        DOUBLE PRECISION NOT NULL,
    tags         JSONB            NOT NULL DEFAULT '{}'::jsonb,
    ts           TIMESTAMPTZ      NOT NULL,
    received_at  TIMESTAMPTZ      NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS metrics_project_name_ts_idx
    ON metrics (project_id, name, ts DESC);
CREATE INDEX IF NOT EXISTS metrics_received_at_idx
    ON metrics (received_at);
