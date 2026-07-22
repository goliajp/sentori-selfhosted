-- Sentori core migration 0031 — saasadmin operator accounts.
--
-- These back `sentori-saas-control`'s login + session auth. They
-- were defined in `saas/migrations/0001_control_plane.sql`, but
-- nothing ever ran that file: saas-control has no `sqlx::migrate!`
-- call, and its own comment says schema is "owned by
-- sentori-server" — which never carried these two tables. The
-- result was a control plane whose login endpoint could only
-- return a 500, so no request could ever be authenticated.
--
-- They live here rather than in a separate control-plane migration
-- set because both binaries share one database; two migration
-- tables racing over the same `_sqlx_migrations` row set is a
-- worse problem than the one it would solve.
--
-- Column shapes are copied verbatim from the saas/ file so the
-- handlers in saas/server/src/handlers/saasadmin.rs bind cleanly.

-- ── saasadmin_users ────────────────────────────────────────────
-- Operator staff for the hosted service. Distinct from `users`:
-- those are workspace members, these are GOLIA-side support.
CREATE TABLE IF NOT EXISTS saasadmin_users (
    id              UUID         PRIMARY KEY,
    email           TEXT         NOT NULL UNIQUE,
    password_hash   TEXT         NOT NULL,
    display_name    TEXT         NOT NULL,
    -- 'staff' | 'super' (super has workspace-delete + cross-
    -- workspace impersonation; staff is read-only).
    role            TEXT         NOT NULL CHECK (role IN ('staff', 'super')),
    created_at      TIMESTAMPTZ  NOT NULL DEFAULT now(),
    last_login_at   TIMESTAMPTZ
);

CREATE UNIQUE INDEX IF NOT EXISTS saasadmin_users_email_ci_idx
    ON saasadmin_users (lower(email));

-- ── saasadmin_sessions ─────────────────────────────────────────
-- Only the SHA-256 of the session token is stored; the raw token
-- is returned once at login and never persisted.
CREATE TABLE IF NOT EXISTS saasadmin_sessions (
    id              UUID         PRIMARY KEY,
    user_id         UUID         NOT NULL REFERENCES saasadmin_users(id) ON DELETE CASCADE,
    token_hash      TEXT         NOT NULL UNIQUE,
    created_at      TIMESTAMPTZ  NOT NULL DEFAULT now(),
    expires_at      TIMESTAMPTZ  NOT NULL,
    user_agent      TEXT,
    ip_addr         TEXT
);

CREATE INDEX IF NOT EXISTS saasadmin_sessions_expires_idx
    ON saasadmin_sessions (expires_at);
CREATE INDEX IF NOT EXISTS saasadmin_sessions_user_idx
    ON saasadmin_sessions (user_id);
