-- Sentori core migration 0010 — notifier (K11) delivery log.
--
-- One row per dispatch attempt. Covers four needs:
--   - **Observability**: "did the alert really go out, when?"
--   - **Dedup**: caller-supplied `dedup_key` UNIQUE so the
--     same logical notification can't be sent twice
--     (`quota-warn-{org}-{date}-90`, `regression-{issue}-{ts}`).
--   - **Retry semantics**: status enum + retries counter so a
--     follow-up retry path can re-INSERT against the same
--     dedup_key without losing prior context.
--   - **Dashboard inbox**: per-project log of recent
--     deliveries with status badges.
--
-- Per K11 design lock 2026-06-21 (4 decisions all
-- "Recommended"):
--   - K11 ships generic `Notifier` + 3 transports + this
--     log; vendor adapters (Slack/Linear/Jira) defer to K12.
--   - lettre for SMTP (legacy choice; STARTTLS + plain modes).
--   - Generic `Notification { subject, body, recipient,
--     metadata }` — K11 stays in transport, not in business
--     event semantics.
--   - delivery_log includes dedup_key + status + retries +
--     error from day one (not a follow-up bolt-on).

-- ── multi-tenancy ──────────────────────────────────────────
-- workspace_id NOT NULL even when project_id IS NULL — system
-- notifications still belong to a workspace (system workspace
-- caller convention). RLS enforces isolation. Dedup index
-- widens to (workspace_id, dedup_key) so two workspaces can
-- pick the same dedup_key without collision.

CREATE TABLE IF NOT EXISTS delivery_log (
    id           UUID         PRIMARY KEY,
    workspace_id UUID         NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    -- NULL allowed: system-level notifications (boot-time
    -- ensure-superadmin, infra alerts) have no project.
    project_id   UUID         REFERENCES projects(id) ON DELETE SET NULL,
    -- Wire form: 'email' | 'webhook' | 'mock'.
    channel     TEXT         NOT NULL,
    -- Email addr OR webhook URL OR mock label.
    recipient   TEXT         NOT NULL,
    subject     TEXT         NOT NULL,
    -- First 500 bytes of the body. The full body lives only
    -- in the transport call — we don't persist potentially
    -- sensitive long-form content to the log.
    body_preview TEXT,
    -- Adapter-specific extras (Slack Block Kit blocks,
    -- webhook signing scheme, lettre headers extra).
    metadata    JSONB        NOT NULL DEFAULT '{}'::jsonb,
    -- 'pending' (inserted, not yet sent) |
    -- 'delivered' (transport ack) |
    -- 'failed' (transport rejected; see `error`).
    status      TEXT         NOT NULL,
    -- 0 on first attempt; incremented on retry path.
    retries     INTEGER      NOT NULL DEFAULT 0,
    -- Error message (truncated to 2 KB) when status = failed.
    error       TEXT,
    -- Caller-supplied; UNIQUE when not NULL → guarantees no
    -- duplicate send for the same logical event.
    dedup_key   TEXT,
    sent_at     TIMESTAMPTZ,
    created_at  TIMESTAMPTZ  NOT NULL DEFAULT now()
);

-- Caller namespaces the dedup_key (e.g.
-- `quota-warn-<org_id>-<date>-90`). Postgres treats NULL as
-- distinct so multiple rows with NULL dedup_key are fine.
-- Index keys on (workspace_id, dedup_key) — two workspaces can
-- reuse the same dedup_key independently.
CREATE UNIQUE INDEX IF NOT EXISTS delivery_log_dedup_idx
    ON delivery_log (workspace_id, dedup_key)
    WHERE dedup_key IS NOT NULL;

-- "Recent deliveries for project P" dashboard query.
CREATE INDEX IF NOT EXISTS delivery_log_project_created_idx
    ON delivery_log (project_id, created_at DESC);

-- "Show me what's still pending" operator query (small set,
-- but partial index keeps it tiny).
CREATE INDEX IF NOT EXISTS delivery_log_pending_idx
    ON delivery_log (created_at)
    WHERE status = 'pending';

ALTER TABLE delivery_log ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS delivery_log_isolation ON delivery_log;
CREATE POLICY delivery_log_isolation ON delivery_log
    USING (workspace_id = current_workspace_id())
    WITH CHECK (workspace_id = current_workspace_id());
