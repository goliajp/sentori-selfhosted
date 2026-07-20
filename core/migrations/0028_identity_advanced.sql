-- Sentori core migration 0028 — identity scopes / fingerprints / merges.
--
-- The identity-scope subsystem powers the GDPR-compliant
-- end-user fingerprinting path:
--   • identity_scopes        — a (name, salt) tuple per workspace
--   • workspace_identity_scopes — workspace ↔ scope link with
--                              `is_default` flag (legacy
--                              `org_identity_scopes`)
--   • identity_fingerprints  — per-event hash row keyed by scope
--   • identity_merges        — operator-driven alias → primary
--                              merge with undo trail
--
-- Column names verbatim from legacy
-- `0065_identity_scopes.sql` + `0066_identity_fingerprints.sql`
-- + `0073_identity_merges.sql`. Legacy `org_id` is renamed to
-- `workspace_id` per top-level alias.
--
-- Note: the legacy `ALTER TABLE projects ADD COLUMN
-- identity_scope_id` from 0074 is NOT replicated here — v0.2
-- handles per-project scope binding via a separate, future
-- migration once the scope-selection UX is finalised.

CREATE TABLE IF NOT EXISTS identity_scopes (
    id         UUID        PRIMARY KEY,
    name       TEXT        NOT NULL,
    salt       BYTEA       NOT NULL CHECK (octet_length(salt) = 32),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS workspace_identity_scopes (
    workspace_id UUID    NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    scope_id     UUID    NOT NULL REFERENCES identity_scopes(id) ON DELETE RESTRICT,
    is_default   BOOLEAN NOT NULL DEFAULT false,
    PRIMARY KEY (workspace_id, scope_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS workspace_identity_scopes_default_idx
    ON workspace_identity_scopes (workspace_id)
    WHERE is_default = true;

CREATE TABLE IF NOT EXISTS identity_fingerprints (
    event_id    UUID        NOT NULL,
    scope_id    UUID        NOT NULL REFERENCES identity_scopes(id) ON DELETE CASCADE,
    key_type    TEXT        NOT NULL,
    fingerprint BYTEA       NOT NULL CHECK (octet_length(fingerprint) = 32),
    received_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (event_id, scope_id, key_type)
);

CREATE INDEX IF NOT EXISTS identity_fingerprints_lookup_idx
    ON identity_fingerprints (scope_id, key_type, fingerprint);
CREATE INDEX IF NOT EXISTS identity_fingerprints_recent_idx
    ON identity_fingerprints (scope_id, received_at DESC);

CREATE TABLE IF NOT EXISTS identity_merges (
    scope_id   UUID        NOT NULL REFERENCES identity_scopes(id) ON DELETE CASCADE,
    primary_fp BYTEA       NOT NULL CHECK (octet_length(primary_fp) = 32),
    alias_fp   BYTEA       NOT NULL CHECK (octet_length(alias_fp)   = 32),
    merged_by  UUID        REFERENCES users(id) ON DELETE SET NULL,
    merged_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    undone_at  TIMESTAMPTZ,
    PRIMARY KEY (scope_id, alias_fp),
    CHECK (alias_fp != primary_fp)
);

CREATE INDEX IF NOT EXISTS identity_merges_active_lookup_idx
    ON identity_merges (scope_id, alias_fp)
    WHERE undone_at IS NULL;
CREATE INDEX IF NOT EXISTS identity_merges_primary_idx
    ON identity_merges (scope_id, primary_fp)
    WHERE undone_at IS NULL;
