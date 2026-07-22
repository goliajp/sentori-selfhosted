-- Sentori core migration 0008 — runtime-metrics (K9).
--
-- Five tables:
--   runtime_metrics_raw       — RANGE-partitioned daily, 90d retention
--   runtime_metrics_1m / _1h / _1d — flat materialized aggregates
--   runtime_metrics_dropped   — per-day accounting counters
--
-- Schema rationale: legacy 0068_runtime_metrics.sql (v2.1 W1)
-- + plan §B K9 ("partition lifecycle + rollup cron").
--
-- Per K9 design lock 2026-06-20:
--   - Per-day partition grain (matches 90d retention; finer
--     than K6 spans's monthly because metrics volume is
--     ~100× per row count).
--   - K9 reuses the K6 PartitionLifecycle pattern via copy
--     adapted to day grain. TODO: extract shared
--     `PartitionLifecycle<Grain>` to a stone when retro-K4
--     events partition lands (3-4 consumers = abstract).
--   - Cascading rollups 1m → 1h → 1d shipped together (1h
--     queries skip raw scan; 1d queries skip 1h scan).

-- ── multi-tenancy ────────────────────────────────────────────
-- workspace_id NOT NULL on all 5 tables, denormalized from
-- projects.workspace_id at INSERT. RLS enforces isolation.
-- Partition keys + rollup PKs stay unchanged — RLS injects
-- workspace_id filter independently of the PK shape.

-- ── runtime_metrics_raw (partitioned) ───────────────────────
-- Composite PK includes (ts, tags_hash) so the partition key
-- is part of uniqueness. `tags_hash` is a stable canonical-
-- JSON hash of `tags` — two batches with the same
-- (project, ts, name, tags) idempotently dedup.
CREATE TABLE IF NOT EXISTS runtime_metrics_raw (
    ts            TIMESTAMPTZ        NOT NULL,
    workspace_id  UUID               NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id    UUID               NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name          TEXT               NOT NULL,
    value         DOUBLE PRECISION   NOT NULL,
    tags          JSONB              NOT NULL DEFAULT '{}'::jsonb,
    tags_hash     BIGINT             NOT NULL,
    -- Denormalized dim columns (also live in `tags`).
    -- Every BI query slices on these — a column lookup is
    -- ~5x cheaper than `tags->>'release'` on cold cache.
    release       TEXT,
    environment   TEXT,
    device_class  TEXT,
    PRIMARY KEY (project_id, ts, name, tags_hash)
) PARTITION BY RANGE (ts);

-- Bootstrap a 5-day window starting from a fixed sentinel
-- so deterministic tests can drive partition-lifecycle
-- assertions without ts-based flakiness.
CREATE TABLE IF NOT EXISTS runtime_metrics_raw_2026_01_01
    PARTITION OF runtime_metrics_raw
    FOR VALUES FROM ('2026-01-01 00:00:00+00') TO ('2026-01-02 00:00:00+00');
CREATE TABLE IF NOT EXISTS runtime_metrics_raw_2026_01_02
    PARTITION OF runtime_metrics_raw
    FOR VALUES FROM ('2026-01-02 00:00:00+00') TO ('2026-01-03 00:00:00+00');
CREATE TABLE IF NOT EXISTS runtime_metrics_raw_2026_01_03
    PARTITION OF runtime_metrics_raw
    FOR VALUES FROM ('2026-01-03 00:00:00+00') TO ('2026-01-04 00:00:00+00');
CREATE TABLE IF NOT EXISTS runtime_metrics_raw_2026_01_04
    PARTITION OF runtime_metrics_raw
    FOR VALUES FROM ('2026-01-04 00:00:00+00') TO ('2026-01-05 00:00:00+00');
CREATE TABLE IF NOT EXISTS runtime_metrics_raw_2026_01_05
    PARTITION OF runtime_metrics_raw
    FOR VALUES FROM ('2026-01-05 00:00:00+00') TO ('2026-01-06 00:00:00+00');
CREATE TABLE IF NOT EXISTS runtime_metrics_raw_default
    PARTITION OF runtime_metrics_raw DEFAULT;

-- Index strategy:
--   (project_id, name, ts DESC) — BI hot path: "metric N
--   for project P over time range". `ts DESC` reads recent
--   buckets first; the partition prune happens via PK before
--   this index even gets consulted.
CREATE INDEX IF NOT EXISTS runtime_metrics_raw_project_name_ts_idx
    ON runtime_metrics_raw (project_id, name, ts DESC);
ALTER TABLE runtime_metrics_raw ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS runtime_metrics_raw_isolation ON runtime_metrics_raw;
CREATE POLICY runtime_metrics_raw_isolation ON runtime_metrics_raw
    USING (workspace_id = current_workspace_id())
    WITH CHECK (workspace_id = current_workspace_id());

-- ── rollup tiers (1m, 1h, 1d) ───────────────────────────────
-- Pre-computed (count, sum, avg, p50, p95, p99) per
-- (project, bucket_ts, name, release, environment,
-- device_class). UPSERT idempotent on the PK so re-running a
-- rollup window produces the same rows.

CREATE TABLE IF NOT EXISTS runtime_metrics_1m (
    bucket_ts     TIMESTAMPTZ        NOT NULL,
    workspace_id  UUID               NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id    UUID               NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name          TEXT               NOT NULL,
    release       TEXT               NOT NULL DEFAULT '',
    environment   TEXT               NOT NULL DEFAULT '',
    device_class  TEXT               NOT NULL DEFAULT '',
    count         BIGINT             NOT NULL,
    sum           DOUBLE PRECISION   NOT NULL,
    avg           DOUBLE PRECISION   NOT NULL,
    p50           DOUBLE PRECISION   NOT NULL,
    p95           DOUBLE PRECISION   NOT NULL,
    p99           DOUBLE PRECISION   NOT NULL,
    PRIMARY KEY (project_id, bucket_ts, name, release, environment, device_class)
);
CREATE INDEX IF NOT EXISTS runtime_metrics_1m_project_name_bucket_idx
    ON runtime_metrics_1m (project_id, name, bucket_ts DESC);
ALTER TABLE runtime_metrics_1m ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS runtime_metrics_1m_isolation ON runtime_metrics_1m;
CREATE POLICY runtime_metrics_1m_isolation ON runtime_metrics_1m
    USING (workspace_id = current_workspace_id())
    WITH CHECK (workspace_id = current_workspace_id());

CREATE TABLE IF NOT EXISTS runtime_metrics_1h (
    bucket_ts     TIMESTAMPTZ        NOT NULL,
    workspace_id  UUID               NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id    UUID               NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name          TEXT               NOT NULL,
    release       TEXT               NOT NULL DEFAULT '',
    environment   TEXT               NOT NULL DEFAULT '',
    device_class  TEXT               NOT NULL DEFAULT '',
    count         BIGINT             NOT NULL,
    sum           DOUBLE PRECISION   NOT NULL,
    avg           DOUBLE PRECISION   NOT NULL,
    p50           DOUBLE PRECISION   NOT NULL,
    p95           DOUBLE PRECISION   NOT NULL,
    p99           DOUBLE PRECISION   NOT NULL,
    PRIMARY KEY (project_id, bucket_ts, name, release, environment, device_class)
);
CREATE INDEX IF NOT EXISTS runtime_metrics_1h_project_name_bucket_idx
    ON runtime_metrics_1h (project_id, name, bucket_ts DESC);
ALTER TABLE runtime_metrics_1h ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS runtime_metrics_1h_isolation ON runtime_metrics_1h;
CREATE POLICY runtime_metrics_1h_isolation ON runtime_metrics_1h
    USING (workspace_id = current_workspace_id())
    WITH CHECK (workspace_id = current_workspace_id());

CREATE TABLE IF NOT EXISTS runtime_metrics_1d (
    bucket_ts     TIMESTAMPTZ        NOT NULL,
    workspace_id  UUID               NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id    UUID               NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name          TEXT               NOT NULL,
    release       TEXT               NOT NULL DEFAULT '',
    environment   TEXT               NOT NULL DEFAULT '',
    device_class  TEXT               NOT NULL DEFAULT '',
    count         BIGINT             NOT NULL,
    sum           DOUBLE PRECISION   NOT NULL,
    avg           DOUBLE PRECISION   NOT NULL,
    p50           DOUBLE PRECISION   NOT NULL,
    p95           DOUBLE PRECISION   NOT NULL,
    p99           DOUBLE PRECISION   NOT NULL,
    PRIMARY KEY (project_id, bucket_ts, name, release, environment, device_class)
);
CREATE INDEX IF NOT EXISTS runtime_metrics_1d_project_name_bucket_idx
    ON runtime_metrics_1d (project_id, name, bucket_ts DESC);
ALTER TABLE runtime_metrics_1d ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS runtime_metrics_1d_isolation ON runtime_metrics_1d;
CREATE POLICY runtime_metrics_1d_isolation ON runtime_metrics_1d
    USING (workspace_id = current_workspace_id())
    WITH CHECK (workspace_id = current_workspace_id());

-- ── runtime_metrics_dropped ─────────────────────────────────
-- Per-day accounting counters. One row per (project, day, reason).
-- Drops sized for ops sanity checks; per-row drop events would
-- be self-DoS.
CREATE TABLE IF NOT EXISTS runtime_metrics_dropped (
    day           DATE   NOT NULL,
    workspace_id  UUID   NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id    UUID   NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    -- 'rate_limit' | 'malformed' | 'invalid_value' | 'over_capacity'
    reason        TEXT   NOT NULL,
    count         BIGINT NOT NULL,
    PRIMARY KEY (project_id, day, reason)
);
ALTER TABLE runtime_metrics_dropped ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS runtime_metrics_dropped_isolation ON runtime_metrics_dropped;
CREATE POLICY runtime_metrics_dropped_isolation ON runtime_metrics_dropped
    USING (workspace_id = current_workspace_id())
    WITH CHECK (workspace_id = current_workspace_id());
