-- Sentori core migration 0006 — push tokens + credentials (K7).
--
-- Two tables:
--
--   push_tokens — one row per (project, native_token) tuple.
--     `native_token` is the raw provider token (APNs hex
--     device id, FCM registration id, web subscription
--     endpoint, etc.). `quarantined_at` non-NULL → token
--     marked dead by a previous send (PermanentlyInvalidToken
--     outcome); dispatcher skips it.
--
--   push_credentials — one row per (project, provider). The
--     non-secret config (key id, project id, env default,
--     vapid public key, …) lives in JSONB `config`; the
--     sensitive bytes (APNs p8, FCM service-account JSON,
--     VAPID private key, HCM client_secret) are stored as
--     S12 secrets-vault sealed envelopes in `secret_blob`.
--     Dispatcher unwraps via S12 at send time.
--
-- Both tables key on a partial unique to enforce "at most
-- one configured credential per provider per project" + "at
-- most one row per (project, native_token)".

-- ── multi-tenancy ───────────────────────────────────────────
-- workspace_id NOT NULL on both, denormalized from
-- projects.workspace_id at INSERT. RLS enforces isolation.

-- ── push_tokens ─────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS push_tokens (
    id              UUID        PRIMARY KEY,
    workspace_id    UUID        NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id      UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    -- Which provider this token belongs to. The provider
    -- decides the wire format of `native_token`.
    kind            TEXT        NOT NULL
                                CHECK (kind IN ('apns', 'fcm', 'webpush', 'hcm', 'mipush')),
    -- The provider-native token (APNs hex, FCM reg id, web
    -- subscription JSON, etc.). Treated as an opaque string
    -- by the dispatcher.
    native_token    TEXT        NOT NULL,
    -- APNs has 'production' / 'sandbox'; others NULL.
    env             TEXT,
    -- Caller-supplied app-side user identifier. Maps to
    -- `payload.user.id` shape used by K4 event-pipeline.
    -- Lets the dispatcher send to "everyone matching user X".
    app_user_id     TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Set when the provider returns PermanentlyInvalidToken.
    -- Dispatcher skips rows where `quarantined_at IS NOT NULL`.
    quarantined_at  TIMESTAMPTZ,
    quarantine_reason TEXT
);
-- A given native_token belongs to one (project, kind) pair —
-- prevents two rows representing the same physical device.
CREATE UNIQUE INDEX IF NOT EXISTS push_tokens_project_kind_token_idx
    ON push_tokens (project_id, kind, native_token);
-- Dispatcher hot path: "live tokens for this (project, kind)".
CREATE INDEX IF NOT EXISTS push_tokens_project_kind_live_idx
    ON push_tokens (project_id, kind)
    WHERE quarantined_at IS NULL;
-- Targeted dispatch: "all tokens for user X across devices".
CREATE INDEX IF NOT EXISTS push_tokens_project_user_idx
    ON push_tokens (project_id, app_user_id)
    WHERE app_user_id IS NOT NULL AND quarantined_at IS NULL;
ALTER TABLE push_tokens ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS push_tokens_isolation ON push_tokens;
CREATE POLICY push_tokens_isolation ON push_tokens
    USING (workspace_id = current_workspace_id())
    WITH CHECK (workspace_id = current_workspace_id());

-- ── push_credentials ────────────────────────────────────────
CREATE TABLE IF NOT EXISTS push_credentials (
    id              UUID        PRIMARY KEY,
    workspace_id    UUID        NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id      UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    kind            TEXT        NOT NULL
                                CHECK (kind IN ('apns', 'fcm', 'webpush', 'hcm', 'mipush')),
    -- Non-secret config — vendor project id, APNs key id,
    -- VAPID public key, etc. Vendor-shape-dependent.
    config          JSONB       NOT NULL DEFAULT '{}'::jsonb,
    -- Sealed envelope (S12 secrets-vault) of the sensitive
    -- bytes (APNs p8, FCM service-account JSON, VAPID private
    -- key, HCM client_secret).
    secret_blob     BYTEA       NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Set by `validate()` calls — UI shows green/red/yellow.
    last_validated_at  TIMESTAMPTZ,
    last_validate_status TEXT
                                CHECK (last_validate_status IN ('ok', 'rejected', 'malformed', 'unreachable', 'not_implemented')
                                       OR last_validate_status IS NULL)
);
-- One credential per (project, provider).
CREATE UNIQUE INDEX IF NOT EXISTS push_credentials_project_kind_idx
    ON push_credentials (project_id, kind);
ALTER TABLE push_credentials ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS push_credentials_isolation ON push_credentials;
CREATE POLICY push_credentials_isolation ON push_credentials
    USING (workspace_id = current_workspace_id())
    WITH CHECK (workspace_id = current_workspace_id());
