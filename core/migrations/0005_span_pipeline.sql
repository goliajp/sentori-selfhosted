-- Sentori core migration 0005 — span-store (K6).
--
-- Owns two tables: spans (RANGE-partitioned monthly by
-- received_at) and traces (UPSERT-keyed rollup, NOT
-- partitioned because the trace_id UNIQUE constraint can't
-- coexist with a partition key — postgres partitioned-table
-- unique indexes must include the partition column).
--
-- This is the FIRST partitioned table in v0.1. K9 runtime-
-- metrics + K4 events (follow-up) will reuse the same shape:
--   - PK includes the partition key column
--   - DEFAULT partition catches strays before the lifecycle
--     task creates the right month
--   - per-month child tables named <table>_YYYY_MM

-- ── multi-tenancy ────────────────────────────────────────────
-- workspace_id NOT NULL on both, denormalized from
-- projects.workspace_id at INSERT. RLS enforces isolation. The
-- partitioned spans table cannot include workspace_id in PK
-- (would force per-partition multi-key recompute); RLS is the
-- isolation mechanism, PK shape stays at (received_at, id).

-- ── spans ────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS spans (
    id              UUID        NOT NULL,
    workspace_id    UUID        NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id      UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    trace_id        UUID        NOT NULL,
    parent_span_id  UUID,
    received_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    started_at      TIMESTAMPTZ NOT NULL,
    duration_ms     INTEGER     NOT NULL,
    op              TEXT        NOT NULL,
    name            TEXT        NOT NULL,
    status          TEXT        NOT NULL CHECK (status IN ('ok', 'error', 'cancelled')),
    tags            JSONB       NOT NULL DEFAULT '{}'::jsonb,
    data            JSONB,
    traceparent     TEXT,
    PRIMARY KEY (received_at, id)
) PARTITION BY RANGE (received_at);

-- Bootstrap 6 monthly partitions starting from a sentinel
-- 2026-01 — the K6 PartitionLifecycle creates further
-- partitions ahead of "now" at runtime, and a janitor cron
-- drops expired ones. The DEFAULT partition catches any row
-- that arrives between calendar months ahead-of-schedule (the
-- only "danger" is a row landing in the DEFAULT partition
-- before the lifecycle has a chance to create the proper
-- one; the row is still queryable and gets re-attached only
-- if the operator runs a manual migration — for v0.1 we
-- accept this and document it).
CREATE TABLE IF NOT EXISTS spans_2026_01 PARTITION OF spans
    FOR VALUES FROM ('2026-01-01') TO ('2026-02-01');
CREATE TABLE IF NOT EXISTS spans_2026_02 PARTITION OF spans
    FOR VALUES FROM ('2026-02-01') TO ('2026-03-01');
CREATE TABLE IF NOT EXISTS spans_2026_03 PARTITION OF spans
    FOR VALUES FROM ('2026-03-01') TO ('2026-04-01');
CREATE TABLE IF NOT EXISTS spans_2026_04 PARTITION OF spans
    FOR VALUES FROM ('2026-04-01') TO ('2026-05-01');
CREATE TABLE IF NOT EXISTS spans_2026_05 PARTITION OF spans
    FOR VALUES FROM ('2026-05-01') TO ('2026-06-01');
CREATE TABLE IF NOT EXISTS spans_2026_06 PARTITION OF spans
    FOR VALUES FROM ('2026-06-01') TO ('2026-07-01');
CREATE TABLE IF NOT EXISTS spans_default PARTITION OF spans DEFAULT;

-- Index strategy:
--   trace_id        → trace detail view (fetch all spans of one trace).
--   parent_span_id  → waterfall build (children of a span); partial
--                     index since every root span has parent_span_id
--                     NULL — saves space.
--   (project_id, received_at DESC) → trace list pagination.
--   (project_id, op) → span search by op (e.g. all http.client spans).
CREATE INDEX IF NOT EXISTS spans_trace_idx
    ON spans (trace_id);
CREATE INDEX IF NOT EXISTS spans_parent_idx
    ON spans (parent_span_id)
    WHERE parent_span_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS spans_project_received_idx
    ON spans (project_id, received_at DESC);
CREATE INDEX IF NOT EXISTS spans_project_op_idx
    ON spans (project_id, op);
ALTER TABLE spans ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS spans_isolation ON spans;
CREATE POLICY spans_isolation ON spans
    USING (workspace_id = current_workspace_id())
    WITH CHECK (workspace_id = current_workspace_id());

-- ── traces (rollup) ─────────────────────────────────────────
-- NOT partitioned: the trace_id UNIQUE index can't include a
-- partition key, so partitioned + ON CONFLICT (trace_id) is
-- incompatible. Trace counts are ~1/200 of span counts in
-- practice, so even a million spans only materialises into
-- a few thousand trace rows; a plain DELETE WHERE last_seen
-- < cutoff (using the index below) is the retention path.
CREATE TABLE IF NOT EXISTS traces (
    trace_id     UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id   UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    -- Root-span fields denormalized for the trace-list view.
    -- NULL until the root span lands (children can arrive
    -- ahead of root in a network race).
    root_op      TEXT,
    root_name    TEXT,
    first_seen   TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen    TIMESTAMPTZ NOT NULL DEFAULT now(),
    span_count   INTEGER NOT NULL DEFAULT 0,
    -- Trace status is worst-of: any span with status='error'
    -- → trace status='error'; otherwise 'cancelled' beats
    -- 'ok'. Maintained by ingest_span's UPSERT.
    status       TEXT NOT NULL DEFAULT 'ok'
                 CHECK (status IN ('ok', 'error', 'cancelled')),
    -- Root-span duration when known; 0 until the root lands.
    -- Wall-clock end-to-end is not honest for async spans;
    -- root duration is the standard tracing convention.
    duration_ms  INTEGER NOT NULL DEFAULT 0
);

-- Trace list keyset pagination: (project_id, last_seen DESC, trace_id DESC).
CREATE INDEX IF NOT EXISTS traces_project_last_seen_idx
    ON traces (project_id, last_seen DESC, trace_id DESC);
-- Retention sweep: DELETE WHERE last_seen < cutoff.
CREATE INDEX IF NOT EXISTS traces_last_seen_idx
    ON traces (last_seen);
ALTER TABLE traces ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS traces_isolation ON traces;
CREATE POLICY traces_isolation ON traces
    USING (workspace_id = current_workspace_id())
    WITH CHECK (workspace_id = current_workspace_id());
