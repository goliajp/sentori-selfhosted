-- Sentori core migration 0002 — auth-session (K2).
--
-- Owns three tables that drive the session + email-verify +
-- password-reset flows:
--
--   1. auth_sessions   — long-lived (30d default) session rows
--      keyed by SHA-256(session_id). Cookie payload is the
--      plaintext session_id wrapped in an S9 SignedCookie;
--      DB stores only the hash so a leaked DB row cannot be
--      replayed as a cookie.
--
--   2. email_verifications — single-use 24h tokens for
--      "click here to verify your email". Same SHA-256
--      pattern as workspace_invites in migration 0001.
--
--   3. password_resets — single-use 2h tokens for "forgot my
--      password" links. Once accepted, every active
--      auth_sessions row for that user is dropped so the
--      rotated password takes effect on every device.
--
-- All three tables fan out from `users.id` (declared in
-- migration 0001). They live in core/ because all three
-- editions (selfhosted / enterprise / saas tenant schema)
-- need them with identical shape — D6 portability says auth
-- rows must round-trip through dump/import unchanged.
--
-- ── multi-tenancy ─────────────────────────────────────────────
-- workspace_id NOT NULL on all three, denormalized from
-- users.workspace_id. RLS enforces cross-workspace isolation.
-- Janitors run as superuser → bypass RLS for bulk expire scans.

-- ── auth_sessions ─────────────────────────────────────────────
-- id_hash is BYTEA(32) = SHA-256 of the plaintext session_id.
-- The plaintext lives only in the client's cookie (wrapped in
-- a SignedCookie); a DB dump never reveals a usable cookie.
CREATE TABLE IF NOT EXISTS auth_sessions (
    id_hash       BYTEA       PRIMARY KEY,
    workspace_id  UUID        NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    user_id       UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at    TIMESTAMPTZ NOT NULL,
    ip            TEXT,
    user_agent    TEXT
);
CREATE INDEX IF NOT EXISTS auth_sessions_user_idx
    ON auth_sessions (user_id);
-- For janitor cron deleting expired rows in bulk. Plain (not
-- partial) index — partial WHERE on now() is non-IMMUTABLE so
-- Postgres rejects it. The full-table index is fine because
-- auth_sessions stays bounded (≤ active sessions per user × users).
CREATE INDEX IF NOT EXISTS auth_sessions_expires_idx
    ON auth_sessions (expires_at);
ALTER TABLE auth_sessions ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS auth_sessions_isolation ON auth_sessions;
CREATE POLICY auth_sessions_isolation ON auth_sessions
    USING (workspace_id = current_workspace_id())
    WITH CHECK (workspace_id = current_workspace_id());

-- ── email_verifications ───────────────────────────────────────
CREATE TABLE IF NOT EXISTS email_verifications (
    id            UUID        PRIMARY KEY,
    workspace_id  UUID        NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    user_id       UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash    BYTEA       NOT NULL UNIQUE,
    expires_at    TIMESTAMPTZ NOT NULL,
    used_at       TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS email_verifications_pending_idx
    ON email_verifications (expires_at) WHERE used_at IS NULL;
ALTER TABLE email_verifications ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS email_verifications_isolation ON email_verifications;
CREATE POLICY email_verifications_isolation ON email_verifications
    USING (workspace_id = current_workspace_id())
    WITH CHECK (workspace_id = current_workspace_id());

-- ── password_resets ───────────────────────────────────────────
CREATE TABLE IF NOT EXISTS password_resets (
    id            UUID        PRIMARY KEY,
    workspace_id  UUID        NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    user_id       UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash    BYTEA       NOT NULL UNIQUE,
    expires_at    TIMESTAMPTZ NOT NULL,
    used_at       TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS password_resets_pending_idx
    ON password_resets (expires_at) WHERE used_at IS NULL;
ALTER TABLE password_resets ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS password_resets_isolation ON password_resets;
CREATE POLICY password_resets_isolation ON password_resets
    USING (workspace_id = current_workspace_id())
    WITH CHECK (workspace_id = current_workspace_id());
