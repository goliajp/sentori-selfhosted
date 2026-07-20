-- Sentori core migration 0015 — billing + usage counters (K17).
--
-- Two tables:
--   workspace_billing — singleton (one row enforced via
--     partial unique index on (1)). plan + stripe_customer_id
--     ref + period boundaries + status.
--   usage_counters — per-(project, period_yyyymm,
--     counter_kind) row. Atomic UPSERT increments + reads.
--
-- v0.1 K1 is single-workspace (no orgs table) so billing
-- attaches at the workspace level. usage counters bucket
-- per-project so dashboards can show "events per project
-- this month" without scanning the events table.
--
-- Stripe webhook ingest is OUT of K17 scope (defer to
-- K17.1 follow-up — needs the S5 stripe-webhook-verify
-- stone + HTTP handler in saas/server). K17 ships the
-- storage shape with `stripe_customer_id` slot so wiring
-- later is just adding the handler.
--
-- Counter kinds (v0.1):
--   events  — K4 captured events
--   spans   — K6 spans
--   replays — K8 replay sessions
--
-- Plan enum lives in code (sentori_billing::Plan) — adding
-- a plan is a code-only change. Schema only stores the
-- string tag.

-- ── multi-tenancy ──────────────────────────────────────────
-- workspace_billing was singleton ((1)) — now keyed on
-- workspace_id (one billing row per workspace). usage_counters
-- denormalizes workspace_id from projects.workspace_id. RLS on
-- both. The singleton index becomes (workspace_id) UNIQUE.

CREATE TABLE IF NOT EXISTS workspace_billing (
    id                   UUID         PRIMARY KEY,
    workspace_id         UUID         NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    -- 'free' | 'pro' | 'enterprise' (Plan::as_db_str)
    plan                 TEXT         NOT NULL DEFAULT 'free',
    -- Stripe Customer object id (`cus_xxx`) once linked.
    stripe_customer_id   TEXT,
    -- 'active' | 'past_due' | 'canceled' | 'trialing' |
    -- 'unpaid' — superset of Stripe Subscription Status so
    -- self-hosted deployments without Stripe can still
    -- represent state. Default 'active' means free-tier
    -- accounts have no billing concern.
    status               TEXT         NOT NULL DEFAULT 'active',
    -- When the current billing period ends. NULL for free
    -- plans (no period).
    current_period_end   TIMESTAMPTZ,
    created_at           TIMESTAMPTZ  NOT NULL DEFAULT now(),
    updated_at           TIMESTAMPTZ  NOT NULL DEFAULT now()
);

-- One billing row per workspace (was global singleton).
CREATE UNIQUE INDEX IF NOT EXISTS workspace_billing_singleton_idx
    ON workspace_billing (workspace_id);

ALTER TABLE workspace_billing ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS workspace_billing_isolation ON workspace_billing;
CREATE POLICY workspace_billing_isolation ON workspace_billing
    USING (workspace_id = current_workspace_id())
    WITH CHECK (workspace_id = current_workspace_id());

CREATE TABLE IF NOT EXISTS usage_counters (
    workspace_id   UUID    NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id     UUID    NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    -- Format: YYYYMM in UTC.
    period_yyyymm  TEXT    NOT NULL CHECK (period_yyyymm ~ '^[0-9]{6}$'),
    -- 'events' | 'spans' | 'replays'
    counter_kind   TEXT    NOT NULL CHECK (counter_kind IN ('events', 'spans', 'replays')),
    count          BIGINT  NOT NULL DEFAULT 0,
    dropped_count  BIGINT  NOT NULL DEFAULT 0,
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (project_id, period_yyyymm, counter_kind)
);

-- "All counters for this period" — dashboard rollup.
CREATE INDEX IF NOT EXISTS usage_counters_period_idx
    ON usage_counters (period_yyyymm);

ALTER TABLE usage_counters ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS usage_counters_isolation ON usage_counters;
CREATE POLICY usage_counters_isolation ON usage_counters
    USING (workspace_id = current_workspace_id())
    WITH CHECK (workspace_id = current_workspace_id());
