-- Sentori core migration 0001 — workspace-identity (K1).
--
-- Owns: workspaces + users + workspace_members + privacy_salts +
--       projects + project_user_visibility + workspace_invites +
--       app_user_identities + audit_logs.
--
-- ── multi-tenancy model (single-DB pivot, 2026-06-22) ──────────
--
-- All tenant-bearing tables carry a workspace_id UUID NOT NULL +
-- a row-level-security policy keyed off the GUC
-- `app.current_workspace`. The application layer pins the GUC at
-- connection / transaction start via the WorkspaceScopedPool
-- (sentori-tenant-scoping). A request that forgets to set the GUC
-- sees zero rows + cannot insert — fail-loud, not fail-silent.
--
-- Superuser / table owner bypass RLS by default (Postgres default
-- semantics); production deploy connects with a non-owner login
-- role so even an app-layer bug can't leak across tenants. The
-- janitor + migration runner connect as superuser intentionally
-- to bypass RLS for schema work + cross-tenant maintenance.
--
-- ── equivalence ───────────────────────────────────────────────
--
-- Same DDL on selfhosted (one workspace, by convention) and SaaS
-- (N workspaces). The D6 data-portability invariant
-- (product-architecture §08.9 + §09.2) is preserved: a SaaS row
-- can dump → workspaces row + denorm scope, replay on selfhosted
-- as workspace #1.
--
-- ── owner uniqueness ──────────────────────────────────────────
--
-- Partial-unique index on (workspace_id) WHERE role='owner' —
-- DB-level guarantee that each workspace holds at most one owner.
-- Owner transfer is a single transaction (DELETE old / UPDATE
-- new) to satisfy this.
--
-- ── privacy salt ──────────────────────────────────────────────
--
-- Project-level NOT NULL (one salt per project). The legacy
-- "org default + project override" two-layer design is gone —
-- every project has its own independent salt so cross-project
-- correlation requires explicit replay (D6 §08.5).
--
-- ── audit log ─────────────────────────────────────────────────
--
-- workspace_id NOT NULL; project_id NULLABLE (workspace-level
-- audit events have no project). System-level audit lives under a
-- reserved "system" workspace (caller's choice; not enforced in
-- DDL).
--
-- The migration is idempotent over `CREATE TABLE IF NOT EXISTS`
-- + `DROP POLICY IF EXISTS` / `CREATE POLICY` to keep dev re-runs
-- safe. sqlx::migrate! tracks application separately via
-- _sqlx_migrations.

CREATE EXTENSION IF NOT EXISTS "pgcrypto"; -- gen_random_uuid (for FK demos; we mint UUIDv7 in Rust)

-- ── RLS helper (STABLE function, planner-friendly) ────────────
-- Reads the per-session GUC `app.current_workspace`. Returns
-- NULL if unset (missing_ok=true + NULLIF on empty string).
-- A NULL return makes the policy expression `workspace_id = NULL`
-- which is never true → all rows rejected. STABLE + PARALLEL SAFE
-- lets the planner evaluate once per query, not per row.
CREATE OR REPLACE FUNCTION current_workspace_id() RETURNS uuid
    LANGUAGE sql STABLE PARALLEL SAFE AS $$
    SELECT NULLIF(current_setting('app.current_workspace', true), '')::uuid
$$;

-- ── workspaces (root of the multi-tenancy graph) ──────────────
-- Every other tenant-bearing row FKs (directly or via denorm) to
-- workspaces.id. selfhosted seeds one row at bootstrap; SaaS
-- creates one per signup.
CREATE TABLE IF NOT EXISTS workspaces (
    id           UUID PRIMARY KEY,
    name         TEXT NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- RLS on workspaces itself: a session pinned to workspace X can
-- only see workspaces.id = X. SuperuserPool (janitor) bypasses.
ALTER TABLE workspaces ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS workspaces_isolation ON workspaces;
CREATE POLICY workspaces_isolation ON workspaces
    USING (id = current_workspace_id())
    WITH CHECK (id = current_workspace_id());

-- ── users (sentori account) ────────────────────────────────────
-- 1:1 user ↔ workspace in v0.1 (user belongs to exactly one
-- workspace). v0.2 may relax to 1:N via a memberships table; the
-- current shape avoids that complexity.
CREATE TABLE IF NOT EXISTS users (
    id              UUID PRIMARY KEY,
    workspace_id    UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    email           TEXT NOT NULL,
    password_hash   TEXT NOT NULL,
    email_verified  BOOLEAN NOT NULL DEFAULT FALSE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- Email is case-insensitively unique globally (not per-workspace).
-- Sentori login flow is email+password → resolve user → resolve
-- workspace; a per-workspace email index would let the same
-- person register the same email twice and fork their account.
CREATE UNIQUE INDEX IF NOT EXISTS users_email_ci_idx
    ON users (LOWER(email));
CREATE INDEX IF NOT EXISTS users_workspace_idx
    ON users (workspace_id);
ALTER TABLE users ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS users_workspace_isolation ON users;
CREATE POLICY users_workspace_isolation ON users
    USING (workspace_id = current_workspace_id())
    WITH CHECK (workspace_id = current_workspace_id());

-- ── workspace_members (RBAC: owner / admin / user) ─────────────
-- workspace_id NOT NULL + (user_id, workspace_id) primary key:
-- a user appears at most once per workspace. v0.1's 1:1 user↔ws
-- model means each user_id appears at most once across ALL rows,
-- but the schema is forward-compatible with v0.2's 1:N model.
CREATE TABLE IF NOT EXISTS workspace_members (
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role         TEXT NOT NULL CHECK (role IN ('owner','admin','user')),
    added_by     UUID REFERENCES users(id) ON DELETE SET NULL,
    added_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, user_id)
);
-- Partial unique index per-workspace: at most one owner per
-- workspace. Owner transfer = single-tx DELETE old / UPDATE new.
CREATE UNIQUE INDEX IF NOT EXISTS workspace_members_one_owner
    ON workspace_members (workspace_id) WHERE role = 'owner';
ALTER TABLE workspace_members ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS workspace_members_isolation ON workspace_members;
CREATE POLICY workspace_members_isolation ON workspace_members
    USING (workspace_id = current_workspace_id())
    WITH CHECK (workspace_id = current_workspace_id());

-- ── privacy_salts (workspace-scoped, project-bound 1:1) ───────
CREATE TABLE IF NOT EXISTS privacy_salts (
    id            UUID PRIMARY KEY,
    workspace_id  UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    salt_bytes    BYTEA NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS privacy_salts_workspace_idx
    ON privacy_salts (workspace_id);
ALTER TABLE privacy_salts ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS privacy_salts_isolation ON privacy_salts;
CREATE POLICY privacy_salts_isolation ON privacy_salts
    USING (workspace_id = current_workspace_id())
    WITH CHECK (workspace_id = current_workspace_id());

-- ── projects ───────────────────────────────────────────────────
-- slug uniqueness becomes per-workspace (was global). Same slug
-- in different workspaces is fine — it scopes to the workspace.
CREATE TABLE IF NOT EXISTS projects (
    id               UUID PRIMARY KEY,
    workspace_id     UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name             TEXT NOT NULL,
    slug             TEXT NOT NULL,
    privacy_salt_id  UUID NOT NULL REFERENCES privacy_salts(id) ON DELETE RESTRICT,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX IF NOT EXISTS projects_workspace_slug_idx
    ON projects (workspace_id, slug);
ALTER TABLE projects ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS projects_isolation ON projects;
CREATE POLICY projects_isolation ON projects
    USING (workspace_id = current_workspace_id())
    WITH CHECK (workspace_id = current_workspace_id());

-- ── project_user_visibility (per-project ACL for 'user' role) ──
-- workspace_id denormalized from projects.workspace_id for RLS.
-- The application's grant flow writes both. Owner/admin auto-see
-- every project — they NEVER appear in this table.
CREATE TABLE IF NOT EXISTS project_user_visibility (
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id   UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    granted_by   UUID REFERENCES users(id) ON DELETE SET NULL,
    granted_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (project_id, user_id)
);
CREATE INDEX IF NOT EXISTS project_user_visibility_user_idx
    ON project_user_visibility (user_id);
ALTER TABLE project_user_visibility ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS project_user_visibility_isolation ON project_user_visibility;
CREATE POLICY project_user_visibility_isolation ON project_user_visibility
    USING (workspace_id = current_workspace_id())
    WITH CHECK (workspace_id = current_workspace_id());

-- ── workspace_invites ─────────────────────────────────────────
-- Token wire format: base64url-no-pad of 32 random bytes
-- (43 chars). Server stores SHA-256(token_bytes) only — a leaked
-- DB row cannot be replayed against the invite-accept endpoint.
-- Single-use: accepted_at is set in the same UPDATE that
-- inserts the workspace_members row; further accept attempts
-- fail the "WHERE accepted_at IS NULL" guard.
CREATE TABLE IF NOT EXISTS workspace_invites (
    id            UUID PRIMARY KEY,
    workspace_id  UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    email         TEXT NOT NULL,
    role          TEXT NOT NULL CHECK (role IN ('admin','user')),
    invited_by    UUID NOT NULL REFERENCES users(id),
    token_hash    BYTEA NOT NULL UNIQUE,
    expires_at    TIMESTAMPTZ NOT NULL,
    accepted_at   TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS workspace_invites_pending_idx
    ON workspace_invites (expires_at) WHERE accepted_at IS NULL;
CREATE INDEX IF NOT EXISTS workspace_invites_workspace_idx
    ON workspace_invites (workspace_id);
ALTER TABLE workspace_invites ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS workspace_invites_isolation ON workspace_invites;
CREATE POLICY workspace_invites_isolation ON workspace_invites
    USING (workspace_id = current_workspace_id())
    WITH CHECK (workspace_id = current_workspace_id());

-- ── app_user_identities (end-user identities seen by SDK) ──────
-- Distinct from `users` (sentori account holders). One sentori
-- project can see millions of app-side end-users (those who
-- triggered an event or session). The privacy-salt namespacing
-- lives on this table's salted_id column (set by the ingest
-- pipeline in K4, not by this crate). workspace_id denormalized
-- from projects.workspace_id for RLS.
CREATE TABLE IF NOT EXISTS app_user_identities (
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id   UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    salted_id    BYTEA NOT NULL,
    first_seen   TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen    TIMESTAMPTZ NOT NULL DEFAULT now(),
    properties   JSONB NOT NULL DEFAULT '{}'::jsonb,
    PRIMARY KEY (project_id, salted_id)
);
ALTER TABLE app_user_identities ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS app_user_identities_isolation ON app_user_identities;
CREATE POLICY app_user_identities_isolation ON app_user_identities
    USING (workspace_id = current_workspace_id())
    WITH CHECK (workspace_id = current_workspace_id());

-- ── audit_logs (workspace-scoped; project_id NULLABLE) ────────
-- workspace_id NOT NULL even for workspace-level events; system
-- events use a reserved "system" workspace UUID (caller's choice).
CREATE TABLE IF NOT EXISTS audit_logs (
    id            UUID PRIMARY KEY,
    workspace_id  UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id    UUID REFERENCES projects(id) ON DELETE SET NULL,
    actor_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    action        TEXT NOT NULL,
    target_type   TEXT,
    target_id     TEXT,
    payload       JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS audit_logs_workspace_created_idx
    ON audit_logs (workspace_id, created_at DESC);
CREATE INDEX IF NOT EXISTS audit_logs_project_created_idx
    ON audit_logs (project_id, created_at DESC);
ALTER TABLE audit_logs ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS audit_logs_isolation ON audit_logs;
CREATE POLICY audit_logs_isolation ON audit_logs
    USING (workspace_id = current_workspace_id())
    WITH CHECK (workspace_id = current_workspace_id());
