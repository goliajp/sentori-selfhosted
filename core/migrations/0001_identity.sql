-- Sentori v1 migration 0001 — identity.
--
-- Fresh sequence for the v1 redesign (docs/plans/design.md). The
-- 0001-0036 v0.2 series lives in git history; production data was
-- explicitly discarded, so nothing migrates forward.
--
-- Single-tenant by design: one Sentori instance is one customer.
-- There is no workspaces table, no RLS, no current_workspace_id().
-- Admin visibility over projects is an application-layer WHERE on
-- project_assignments (0002), not a database policy.
--
-- Roles: exactly two. The owner (superadmin) is declared by env at
-- boot (SENTORI_OWNER_EMAIL / _PASSWORD) and reconciled on every
-- start; admins are created by the owner. There is no self-signup,
-- so there is no email_verified column — an account created by the
-- owner is trusted by construction. Password resets stay: they are
-- the one auth email flow the product keeps (design.md §10).

CREATE TABLE users (
    id            UUID        PRIMARY KEY,
    email         TEXT        NOT NULL,
    password_hash TEXT        NOT NULL,
    role          TEXT        NOT NULL CHECK (role IN ('superadmin', 'admin')),
    display_name  TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_login_at TIMESTAMPTZ
);
CREATE UNIQUE INDEX users_email_ci_idx ON users (LOWER(email));

-- Cookie sessions. id_hash is the SHA-256 of the opaque session id
-- that lives in the cookie — a DB dump alone cannot forge a session.
CREATE TABLE auth_sessions (
    id_hash      BYTEA       PRIMARY KEY,
    user_id      UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at   TIMESTAMPTZ NOT NULL,
    ip           TEXT,
    user_agent   TEXT
);
CREATE INDEX auth_sessions_user_idx ON auth_sessions (user_id);
CREATE INDEX auth_sessions_expires_idx ON auth_sessions (expires_at);

CREATE TABLE password_resets (
    id         UUID        PRIMARY KEY,
    user_id    UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash BYTEA       NOT NULL UNIQUE,
    expires_at TIMESTAMPTZ NOT NULL,
    used_at    TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX password_resets_pending_idx
    ON password_resets (expires_at) WHERE used_at IS NULL;

-- Append-only admin audit. project_id deliberately carries no FK:
-- the record of "who deleted project X" must outlive project X, and
-- audit rows are never joined for correctness, only filtered.
CREATE TABLE audit_logs (
    id            UUID        PRIMARY KEY,
    project_id    UUID,
    actor_user_id UUID        REFERENCES users(id) ON DELETE SET NULL,
    action        TEXT        NOT NULL,
    target_type   TEXT,
    target_id     TEXT,
    payload       JSONB       NOT NULL DEFAULT '{}'::jsonb,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX audit_logs_created_idx ON audit_logs (created_at DESC);
CREATE INDEX audit_logs_project_created_idx
    ON audit_logs (project_id, created_at DESC);
