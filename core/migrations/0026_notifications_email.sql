-- Sentori core migration 0026 — email + webhook delivery surfaces
-- (digest_subscriptions + digest_runs + webhook_deliveries +
--  notification_preferences + notifications_email_log).
--
-- Together these track the "what / when / to-whom" of every
-- outbound message the dashboard sends on behalf of a user:
--   • digest_subscriptions / digest_runs — daily / hourly digest
--   • webhook_deliveries  — outbound webhook attempt log
--   • notification_preferences — per-user cadence + channels
--   • notifications_email_log — per-row email send result
--
-- Legacy uses `org_id`; v0.2 renames to `workspace_id`
-- (top-level alias). All other column names verbatim.
--
-- Source migrations:
--   digest_subscriptions      — 0023_digest_subscriptions.sql
--   webhook_deliveries        — 0025_webhook_deliveries.sql
--   notification_preferences  — 0056_notification_preferences.sql
--   notifications_email_log   — 0058_notifications_email_log.sql
--   digest_runs               — 0059_digest_runs.sql

CREATE TABLE IF NOT EXISTS digest_subscriptions (
    user_id      UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    workspace_id UUID        NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    frequency    TEXT        NOT NULL CHECK (frequency IN ('daily', 'weekly')),
    last_sent_at TIMESTAMPTZ,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, workspace_id, frequency)
);

CREATE INDEX IF NOT EXISTS digest_subscriptions_due_idx
    ON digest_subscriptions (frequency, last_sent_at);

CREATE TABLE IF NOT EXISTS digest_runs (
    user_id      UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    cadence      TEXT        NOT NULL CHECK (cadence IN ('hourly', 'daily')),
    last_sent_at TIMESTAMPTZ,
    PRIMARY KEY (user_id, cadence)
);

CREATE TABLE IF NOT EXISTS webhook_deliveries (
    id              UUID        PRIMARY KEY,
    rule_id         UUID        NOT NULL REFERENCES alert_rules(id) ON DELETE CASCADE,
    payload         JSONB       NOT NULL,
    target_url      TEXT        NOT NULL,
    secret          TEXT        NOT NULL,
    attempt         INTEGER     NOT NULL DEFAULT 0,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_status     INTEGER,
    last_error      TEXT,
    status          TEXT        NOT NULL DEFAULT 'pending'
                                CHECK (status IN ('pending', 'delivered', 'failed')),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    delivered_at    TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS webhook_deliveries_pending_idx
    ON webhook_deliveries (status, next_attempt_at)
    WHERE status = 'pending';
CREATE INDEX IF NOT EXISTS webhook_deliveries_rule_recent_idx
    ON webhook_deliveries (rule_id, created_at DESC);

CREATE TABLE IF NOT EXISTS notification_preferences (
    user_id     UUID        PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    muted_kinds TEXT[]      NOT NULL DEFAULT ARRAY[]::TEXT[],
    cadence     TEXT        NOT NULL DEFAULT 'immediate'
                            CHECK (cadence IN ('immediate', 'hourly', 'daily')),
    channels    TEXT[]      NOT NULL DEFAULT ARRAY['in_app']::TEXT[],
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS notifications_email_log (
    id              BIGSERIAL   PRIMARY KEY,
    notification_id BIGINT      REFERENCES notifications(id) ON DELETE CASCADE,
    user_id         UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    recipient_email TEXT        NOT NULL,
    status          TEXT        NOT NULL DEFAULT 'queued'
                                CHECK (status IN ('queued', 'delivered', 'failed', 'skipped')),
    subject         TEXT        NOT NULL,
    last_error      TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    delivered_at    TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS notifications_email_log_user_idx
    ON notifications_email_log (user_id, created_at DESC);
