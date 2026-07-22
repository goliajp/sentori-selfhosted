-- 0036 — give every workspace an identity scope.
--
-- `identity_scopes` and `identity_fingerprints` came across from the v1
-- stack with 338 rows of real fingerprints, but `workspace_identity_scopes`
-- — the table that says which scope a workspace hashes against — was
-- never populated during the v0.2 cutover. Ingest therefore had nothing
-- to resolve, and stopped writing fingerprints entirely: PII-safe
-- cross-project user lookup has been quietly dead since 2026-07-19,
-- while the SDK kept sending `user.linkHashes` on every event.
--
-- Two steps, in order:
--
--   1. Adopt the scope that already owns this workspace's existing
--      fingerprints. Minting a fresh salt instead would re-hash the same
--      people to different values and orphan every row already stored —
--      the person would stop matching their own history, silently.
--   2. Mint a scope only for workspaces that have no fingerprints yet,
--      where there is no history to preserve.
--
-- Idempotent: re-running adopts nothing new and mints nothing twice.

-- 1. Adopt the scope this workspace's fingerprints were written under.
INSERT INTO workspace_identity_scopes (workspace_id, scope_id, is_default)
SELECT DISTINCT w.id, f.scope_id, true
FROM workspaces w
JOIN projects p ON p.workspace_id = w.id
JOIN events e ON e.project_id = p.id
JOIN identity_fingerprints f ON f.event_id = e.id
ON CONFLICT (workspace_id, scope_id) DO NOTHING;

-- 2. Mint one for the rest.
--
-- `gen_random_bytes` needs pgcrypto; `sha256(random()::text)` is the
-- portable stand-in and is only a seed — the salt's job is to be
-- unguessable per scope, and it never leaves the database.
WITH unscoped AS (
    SELECT w.id
    FROM workspaces w
    LEFT JOIN workspace_identity_scopes s ON s.workspace_id = w.id
    WHERE s.workspace_id IS NULL
),
minted AS (
    INSERT INTO identity_scopes (id, name, salt, created_at)
    SELECT
        gen_random_uuid(),
        'workspace ' || u.id::text,
        sha256((u.id::text || clock_timestamp()::text || random()::text)::bytea),
        now()
    FROM unscoped u
    RETURNING id, name
)
INSERT INTO workspace_identity_scopes (workspace_id, scope_id, is_default)
SELECT
    replace(m.name, 'workspace ', '')::uuid,
    m.id,
    true
FROM minted m
ON CONFLICT (workspace_id, scope_id) DO NOTHING;
