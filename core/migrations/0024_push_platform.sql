-- Sentori core migration 0024 — push platform (device_tokens +
-- push_sends + push_delivery_logs + device_topics + push_preferences).
--
-- This is the legacy-named push-handle + dispatch-log surface
-- needed by the SaaS ETL. The v0.1 fresh-design also has
-- `push_tokens` / `push_credentials` in `0006_push_tokens.sql`
-- (sentori-core architecture); the two layers coexist by design
-- — `push_tokens` is the K7 dispatch primitive, `device_tokens`
-- is the legacy-named row the dashboard + ETL read from.
--
-- Columns verbatim from legacy:
--   device_tokens         — 0075_device_tokens.sql
--   push_sends            — 0077_push_sends.sql + 0079 (campaign /
--                           template / audience cols) + 0080 (ack cols)
--   push_delivery_logs    — 0078_push_delivery_logs.sql
--   device_topics         — 0081_device_topics.sql
--   push_preferences      — 0082_push_preferences.sql

CREATE TABLE IF NOT EXISTS device_tokens (
    id                    UUID        PRIMARY KEY,
    workspace_id          UUID        NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id            UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    provider              TEXT        NOT NULL CHECK (provider IN ('apns','fcm','webpush','hcm','mipush')),
    env                   TEXT        CHECK (env IN ('sandbox','production')),
    native_token          TEXT        NOT NULL,
    user_fingerprint_hex  BYTEA       CHECK (user_fingerprint_hex IS NULL OR octet_length(user_fingerprint_hex) = 32),
    metadata              JSONB       NOT NULL DEFAULT '{}'::jsonb,
    bad_streak            INTEGER     NOT NULL DEFAULT 0,
    revoked_at            TIMESTAMPTZ,
    last_seen_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, provider, native_token)
);

CREATE INDEX IF NOT EXISTS device_tokens_project_active_idx
    ON device_tokens (project_id)
    WHERE revoked_at IS NULL;
CREATE INDEX IF NOT EXISTS device_tokens_user_active_idx
    ON device_tokens (user_fingerprint_hex)
    WHERE revoked_at IS NULL AND user_fingerprint_hex IS NOT NULL;

CREATE TABLE IF NOT EXISTS push_sends (
    id                UUID        PRIMARY KEY,
    workspace_id      UUID        NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id        UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    token_id          UUID        NOT NULL REFERENCES device_tokens(id) ON DELETE CASCADE,
    provider          TEXT        NOT NULL,
    payload           JSONB       NOT NULL,
    status            TEXT        NOT NULL DEFAULT 'queued'
                                  CHECK (status IN ('queued','sent','failed')),
    provider_outcome  TEXT,
    error             TEXT,
    retry_count       INTEGER     NOT NULL DEFAULT 0,
    idempotency_key   TEXT,
    next_attempt_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    sent_at           TIMESTAMPTZ,
    campaign_id       TEXT,
    template_id       TEXT,
    audience_tag      TEXT,
    acked_at          TIMESTAMPTZ,
    ack_session_id    TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS push_sends_idempotency_idx
    ON push_sends (project_id, idempotency_key)
    WHERE idempotency_key IS NOT NULL;
CREATE INDEX IF NOT EXISTS push_sends_pending_idx
    ON push_sends (next_attempt_at)
    WHERE status = 'queued';
CREATE INDEX IF NOT EXISTS push_sends_token_recent_idx
    ON push_sends (token_id, created_at DESC);
CREATE INDEX IF NOT EXISTS push_sends_campaign_idx
    ON push_sends (project_id, campaign_id, created_at DESC)
    WHERE campaign_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS push_sends_acked_idx
    ON push_sends (project_id, acked_at)
    WHERE acked_at IS NOT NULL;

CREATE TABLE IF NOT EXISTS push_delivery_logs (
    id               UUID        PRIMARY KEY,
    send_id          UUID        NOT NULL REFERENCES push_sends(id) ON DELETE CASCADE,
    attempt          INTEGER     NOT NULL,
    outcome          TEXT        NOT NULL,
    provider_status  INTEGER,
    provider_body    TEXT,
    duration_ms      INTEGER,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS push_delivery_logs_send_idx
    ON push_delivery_logs (send_id, attempt);

CREATE TABLE IF NOT EXISTS device_topics (
    device_token_id UUID        NOT NULL REFERENCES device_tokens(id) ON DELETE CASCADE,
    topic           TEXT        NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (device_token_id, topic)
);

CREATE INDEX IF NOT EXISTS device_topics_topic_idx
    ON device_topics (topic);

CREATE TABLE IF NOT EXISTS push_preferences (
    project_id            UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    user_fingerprint_hex  BYTEA       NOT NULL,
    category              TEXT        NOT NULL,
    opted_out             BOOLEAN     NOT NULL DEFAULT false,
    updated_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (project_id, user_fingerprint_hex, category)
);
