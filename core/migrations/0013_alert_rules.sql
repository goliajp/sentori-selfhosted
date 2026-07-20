-- Sentori core migration 0013 — alert rules (K14).
--
-- One row per rule. project_id NULL = workspace-wide rule that
-- fires for every project. Trigger kinds + filter shapes are
-- JSONB-driven so adding a new shape is code-only (CHECK on
-- `trigger_kind` keeps the enum surface tight).
--
-- Trigger kinds (v0.1):
--   - new_issue        — first event of a fingerprint
--   - regression       — resolved → regressed flip
--   - event_count      — ≥N events match filter in `windowMinutes`
--   - crash_free_drop  — crash-free session rate dips below
--                        `threshold` in the last `windowMinutes`
--
-- Trigger config shapes (camelCase, K14 evaluator parses):
--   new_issue        : {}
--   regression       : {}
--   event_count      : { count: 100, windowMinutes: 5 }
--   crash_free_drop  : { threshold: 0.99, windowMinutes: 60 }
--
-- Filter config: { environment?, release?, errorType? } —
-- exact-match semantics (no regex in v0.1; defer to a future
-- migration if needed).
--
-- Channels: opaque JSONB array. Caller (saas/server) maps to
-- K11 Notifications when dispatching. Examples:
--   [ { "type": "email", "to": ["a@x.com"] },
--     { "type": "webhook", "url": "https://...", "secret": "..." } ]
--
-- Throttle is enforced atomically via the same UPDATE that
-- records last_fired_at = now() — a row is "claimed" for
-- firing only when the WHERE clause sees the throttle window
-- elapsed. Two evaluators racing won't double-page.
--
-- Per K14 design (autonomous, 2026-06-21):
--   - Sentori v0.1 K1 is single-workspace (no orgs table) →
--     no org_id column. project_id NULL = workspace-wide.
--   - muted + snoozed_until shipped from day one (legacy
--     added them later; including now avoids a follow-up).

-- ── multi-tenancy ──────────────────────────────────────────
-- workspace_id NOT NULL even when project_id IS NULL
-- (workspace-wide rule still belongs to a workspace). RLS
-- enforces isolation. The "workspace-wide" partial index
-- widens from ((1)) singleton to (workspace_id) so each
-- workspace can hold its own collection of workspace-wide
-- rules.

CREATE TABLE IF NOT EXISTS alert_rules (
    id               UUID         PRIMARY KEY,
    workspace_id     UUID         NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id       UUID         REFERENCES projects(id) ON DELETE CASCADE,
    name             TEXT         NOT NULL,
    enabled          BOOLEAN      NOT NULL DEFAULT TRUE,

    trigger_kind     TEXT         NOT NULL CHECK (trigger_kind IN
        ('new_issue', 'regression', 'event_count', 'crash_free_drop')),
    trigger_config   JSONB        NOT NULL DEFAULT '{}'::jsonb,
    filter_config    JSONB        NOT NULL DEFAULT '{}'::jsonb,
    channels         JSONB        NOT NULL DEFAULT '[]'::jsonb,

    throttle_minutes INTEGER      NOT NULL DEFAULT 10 CHECK (throttle_minutes >= 0),
    last_fired_at    TIMESTAMPTZ,

    -- Operator-driven silence controls.
    muted            BOOLEAN      NOT NULL DEFAULT FALSE,
    snoozed_until    TIMESTAMPTZ,

    created_at       TIMESTAMPTZ  NOT NULL DEFAULT now(),
    created_by       UUID         REFERENCES users(id) ON DELETE SET NULL,
    updated_at       TIMESTAMPTZ  NOT NULL DEFAULT now()
);

-- "Show me every rule for project P" — workspace-wide rules
-- (project_id NULL) come through the explicit JOIN in queries,
-- not via this index.
CREATE INDEX IF NOT EXISTS alert_rules_project_idx
    ON alert_rules (project_id)
    WHERE project_id IS NOT NULL;

-- "Find every workspace-wide rule" for THIS workspace — was
-- ((1)) singleton, now keyed on workspace_id so it scales.
CREATE INDEX IF NOT EXISTS alert_rules_workspace_wide_idx
    ON alert_rules (workspace_id)
    WHERE project_id IS NULL;

-- Evaluator hot path: "every enabled rule of kind X due for
-- firing". Partial index trims to active rules.
CREATE INDEX IF NOT EXISTS alert_rules_active_kind_idx
    ON alert_rules (trigger_kind, last_fired_at NULLS FIRST)
    WHERE enabled = TRUE AND muted = FALSE;

ALTER TABLE alert_rules ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS alert_rules_isolation ON alert_rules;
CREATE POLICY alert_rules_isolation ON alert_rules
    USING (workspace_id = current_workspace_id())
    WITH CHECK (workspace_id = current_workspace_id());
