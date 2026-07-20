-- Sentori core migration 0029 — endpoint health monitoring.
--
-- A workspace-defined set of HTTP `endpoint_check` rows that
-- the prober schedules at `interval_sec`. Each probe attempt
-- writes a row into the time-partitioned `endpoint_probe` table;
-- the hourly rollup lives in `endpoint_probe_1h` for fast
-- uptime-percent display.
--
-- Column names verbatim from legacy
-- `0070_endpoint_checks.sql` + `0071_endpoint_probes.sql` +
-- `0072_endpoint_probe_rollup.sql`. The seed-partition
-- statements from 0071 are intentionally omitted — partition
-- creation is the runtime prober's responsibility, not a
-- schema concern. The DEFAULT partition catches stray writes.

CREATE TABLE IF NOT EXISTS endpoint_check (
    id                       UUID        PRIMARY KEY,
    workspace_id             UUID        NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id               UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name                     TEXT        NOT NULL,
    target_url               TEXT        NOT NULL,
    method                   TEXT        NOT NULL DEFAULT 'GET',
    interval_sec             INTEGER     NOT NULL DEFAULT 60 CHECK (interval_sec >= 60),
    assertion_status_codes   INTEGER[]   NOT NULL DEFAULT ARRAY[200],
    assertion_body_substring TEXT,
    assertion_max_latency_ms INTEGER,
    paused                   BOOLEAN     NOT NULL DEFAULT false,
    created_by               UUID        REFERENCES users(id) ON DELETE SET NULL,
    created_at               TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at               TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS endpoint_check_project_idx
    ON endpoint_check (project_id);
CREATE INDEX IF NOT EXISTS endpoint_check_active_idx
    ON endpoint_check (project_id, interval_sec)
    WHERE NOT paused;

CREATE TABLE IF NOT EXISTS endpoint_probe (
    ts          TIMESTAMPTZ NOT NULL,
    check_id    UUID        NOT NULL,
    status_code INTEGER     NOT NULL,
    latency_ms  INTEGER     NOT NULL,
    ok          BOOLEAN     NOT NULL,
    error_kind  TEXT,
    PRIMARY KEY (check_id, ts)
) PARTITION BY RANGE (ts);

-- DEFAULT partition catches probes outside the prober's
-- pre-created daily partitions. Operationally this should stay
-- empty; non-empty = partition-runner bug.
CREATE TABLE IF NOT EXISTS endpoint_probe_default
    PARTITION OF endpoint_probe DEFAULT;

CREATE INDEX IF NOT EXISTS endpoint_probe_check_ts_idx
    ON endpoint_probe (check_id, ts DESC);

CREATE TABLE IF NOT EXISTS endpoint_probe_1h (
    bucket_ts      TIMESTAMPTZ      NOT NULL,
    check_id       UUID             NOT NULL,
    probe_count    INTEGER          NOT NULL,
    ok_count       INTEGER          NOT NULL,
    uptime_pct     DOUBLE PRECISION NOT NULL,
    p50_latency_ms INTEGER          NOT NULL,
    p95_latency_ms INTEGER          NOT NULL,
    PRIMARY KEY (check_id, bucket_ts)
);

CREATE INDEX IF NOT EXISTS endpoint_probe_1h_check_bucket_idx
    ON endpoint_probe_1h (check_id, bucket_ts DESC);
