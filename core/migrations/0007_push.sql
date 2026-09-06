-- Sentori v1 migration 0007 — push notification tables.
--
-- Carried over mechanically from the v0.2 series (0006_push_tokens +
-- 0024_push_platform) minus the workspace columns and RLS. Push's
-- product fate is an open item in the design ledger (design.md §12,
-- "第四条腿"); the code is live in production and stays working, but
-- push is outside the v1 acceptance scope. No schema redesign here —
-- just the single-tenant transcription.

CREATE TABLE push_tokens (
    id                UUID        PRIMARY KEY,
    project_id        UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    kind              TEXT        NOT NULL
                                  CHECK (kind IN ('apns', 'fcm', 'webpush', 'hcm', 'mipush')),
    native_token      TEXT        NOT NULL,
    env               TEXT,
    app_user_id       TEXT,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    quarantined_at    TIMESTAMPTZ,
    quarantine_reason TEXT
);
CREATE UNIQUE INDEX push_tokens_project_kind_token_idx
    ON push_tokens (project_id, kind, native_token);
CREATE INDEX push_tokens_project_kind_live_idx
    ON push_tokens (project_id, kind) WHERE quarantined_at IS NULL;
CREATE INDEX push_tokens_project_user_idx
    ON push_tokens (project_id, app_user_id)
    WHERE app_user_id IS NOT NULL AND quarantined_at IS NULL;

CREATE TABLE push_credentials (
    id                   UUID        PRIMARY KEY,
    project_id           UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    kind                 TEXT        NOT NULL
                                     CHECK (kind IN ('apns', 'fcm', 'webpush', 'hcm', 'mipush')),
    config               JSONB       NOT NULL DEFAULT '{}'::jsonb,
    secret_blob          BYTEA       NOT NULL,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_validated_at    TIMESTAMPTZ,
    last_validate_status TEXT
                         CHECK (last_validate_status IN
                                ('ok', 'rejected', 'malformed', 'unreachable', 'not_implemented')
                                OR last_validate_status IS NULL)
);
CREATE UNIQUE INDEX push_credentials_project_kind_idx
    ON push_credentials (project_id, kind);

CREATE TABLE device_tokens (
    id                   UUID        PRIMARY KEY,
    project_id           UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    provider             TEXT        NOT NULL
                                     CHECK (provider IN ('apns', 'fcm', 'webpush', 'hcm', 'mipush')),
    env                  TEXT        CHECK (env IN ('sandbox', 'production')),
    native_token         TEXT        NOT NULL,
    user_fingerprint_hex BYTEA       CHECK (user_fingerprint_hex IS NULL
                                            OR octet_length(user_fingerprint_hex) = 32),
    metadata             JSONB       NOT NULL DEFAULT '{}'::jsonb,
    bad_streak           INTEGER     NOT NULL DEFAULT 0,
    revoked_at           TIMESTAMPTZ,
    last_seen_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, provider, native_token)
);
CREATE INDEX device_tokens_project_active_idx
    ON device_tokens (project_id) WHERE revoked_at IS NULL;
CREATE INDEX device_tokens_user_active_idx
    ON device_tokens (user_fingerprint_hex)
    WHERE revoked_at IS NULL AND user_fingerprint_hex IS NOT NULL;

CREATE TABLE push_sends (
    id               UUID        PRIMARY KEY,
    project_id       UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    token_id         UUID        NOT NULL REFERENCES device_tokens(id) ON DELETE CASCADE,
    provider         TEXT        NOT NULL,
    payload          JSONB       NOT NULL,
    status           TEXT        NOT NULL DEFAULT 'queued'
                                 CHECK (status IN ('queued', 'sent', 'failed')),
    provider_outcome TEXT,
    error            TEXT,
    retry_count      INTEGER     NOT NULL DEFAULT 0,
    idempotency_key  TEXT,
    next_attempt_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    sent_at          TIMESTAMPTZ,
    campaign_id      TEXT,
    template_id      TEXT,
    audience_tag     TEXT,
    acked_at         TIMESTAMPTZ,
    ack_session_id   TEXT
);
CREATE UNIQUE INDEX push_sends_idempotency_idx
    ON push_sends (project_id, idempotency_key) WHERE idempotency_key IS NOT NULL;
CREATE INDEX push_sends_pending_idx
    ON push_sends (next_attempt_at) WHERE status = 'queued';
CREATE INDEX push_sends_token_recent_idx
    ON push_sends (token_id, created_at DESC);
CREATE INDEX push_sends_campaign_idx
    ON push_sends (project_id, campaign_id, created_at DESC)
    WHERE campaign_id IS NOT NULL;
CREATE INDEX push_sends_acked_idx
    ON push_sends (project_id, acked_at) WHERE acked_at IS NOT NULL;

CREATE TABLE push_delivery_logs (
    id              UUID        PRIMARY KEY,
    send_id         UUID        NOT NULL REFERENCES push_sends(id) ON DELETE CASCADE,
    attempt         INTEGER     NOT NULL,
    outcome         TEXT        NOT NULL,
    provider_status INTEGER,
    provider_body   TEXT,
    duration_ms     INTEGER,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX push_delivery_logs_send_idx
    ON push_delivery_logs (send_id, attempt);

CREATE TABLE device_topics (
    device_token_id UUID        NOT NULL REFERENCES device_tokens(id) ON DELETE CASCADE,
    topic           TEXT        NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (device_token_id, topic)
);
CREATE INDEX device_topics_topic_idx ON device_topics (topic);

CREATE TABLE push_preferences (
    project_id           UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    user_fingerprint_hex BYTEA       NOT NULL,
    category             TEXT        NOT NULL,
    opted_out            BOOLEAN     NOT NULL DEFAULT false,
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (project_id, user_fingerprint_hex, category)
);
