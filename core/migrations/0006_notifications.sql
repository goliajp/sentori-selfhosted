-- Sentori v1 migration 0006 — notification preferences + delivery log.
--
-- Channels subscribe to the issue system (design.md §9): email is
-- the first, Jira later. Per-user per-project opt-outs; the default
-- (no row) is "notify on both", so a fresh install with SMTP
-- configured notifies without further setup.
--
-- delivery_log is the notifier crate's table, carried over with the
-- workspace column (and its dedup-index prefix) removed — dedup_key
-- is globally unique now, which is what single-tenant means.

CREATE TABLE notification_prefs (
    user_id       UUID    NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    project_id    UUID    NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    on_new_issue  BOOLEAN NOT NULL DEFAULT TRUE,
    on_regression BOOLEAN NOT NULL DEFAULT TRUE,
    PRIMARY KEY (user_id, project_id)
);

CREATE TABLE delivery_log (
    id           UUID        PRIMARY KEY,
    -- NULL allowed: system-level notifications (boot-time owner
    -- bootstrap, infra alerts) have no project.
    project_id   UUID        REFERENCES projects(id) ON DELETE SET NULL,
    -- Wire form: 'email' | 'webhook' | 'mock'.
    channel      TEXT        NOT NULL,
    -- Email addr OR webhook URL OR mock label.
    recipient    TEXT        NOT NULL,
    subject      TEXT        NOT NULL,
    -- First 500 bytes of the body; the full body lives only in the
    -- transport call — potentially sensitive long-form content is
    -- not persisted to the log.
    body_preview TEXT,
    -- Adapter-specific extras (webhook signing scheme, lettre
    -- headers extra).
    metadata     JSONB       NOT NULL DEFAULT '{}'::jsonb,
    -- 'pending' | 'delivered' | 'failed' (see `error`).
    status       TEXT        NOT NULL,
    retries      INTEGER     NOT NULL DEFAULT 0,
    error        TEXT,
    -- Caller-supplied; UNIQUE when not NULL → no duplicate send for
    -- the same logical event.
    dedup_key    TEXT,
    sent_at      TIMESTAMPTZ,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX delivery_log_dedup_idx
    ON delivery_log (dedup_key) WHERE dedup_key IS NOT NULL;
CREATE INDEX delivery_log_project_created_idx
    ON delivery_log (project_id, created_at DESC);
CREATE INDEX delivery_log_pending_idx
    ON delivery_log (created_at) WHERE status = 'pending';
