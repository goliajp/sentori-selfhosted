-- Sentori core migration 0033 — multi-workspace (1:N) foundation.
--
-- v0.1 was 1:1 user↔workspace: `session_mw` resolved a login's
-- workspace straight from `users.workspace_id`, and every SaaS
-- self-signup landed in the same `DEFAULT_WORKSPACE_ID`, so
-- tenants could see each other's data. v0.2 goes 1:N (a user can
-- belong to / switch between many workspaces), which needs two
-- things this migration establishes:
--
-- 1. `workspace_members` becomes the source of truth for "which
--    workspaces can this user reach". `auth_sessions.workspace_id`
--    (already NOT NULL since 0002) becomes the *active* workspace
--    for a session — switching is an UPDATE of that column, and
--    `session_mw` validates the pair against `workspace_members`.
--    No schema change is needed for the active-workspace column;
--    it already exists.
--
-- 2. Backfill: `AuthService::register` historically wrote a `users`
--    row WITHOUT a matching `workspace_members` row (only
--    `bootstrap.rs` seeded the owner for the default workspace).
--    So existing accounts have no membership and would be locked
--    out the moment `session_mw` starts requiring one. Seed an
--    `owner` membership for every user that lacks any membership,
--    scoped to their `users.workspace_id` (their home workspace).

-- Owner backfill. `added_by = user_id` (self-provisioned).
--
-- One owner per workspace: the partial unique index
-- `workspace_members_one_owner` rejects a second owner. Because a
-- single INSERT...SELECT evaluates its WHERE against the pre-insert
-- state, a workspace with two membership-less users and no existing
-- owner would emit two owner rows in one statement and trip the
-- index — so pick exactly ONE (the earliest-created user) per
-- ownerless workspace with DISTINCT ON. Everyone else is handled by
-- the admin backfill below.
INSERT INTO workspace_members (workspace_id, user_id, role, added_by)
SELECT DISTINCT ON (u.workspace_id) u.workspace_id, u.id, 'owner', u.id
FROM users u
WHERE NOT EXISTS (
    SELECT 1 FROM workspace_members m WHERE m.user_id = u.id
)
AND NOT EXISTS (
    SELECT 1 FROM workspace_members o
    WHERE o.workspace_id = u.workspace_id AND o.role = 'owner'
)
ORDER BY u.workspace_id, u.created_at ASC
ON CONFLICT (workspace_id, user_id) DO NOTHING;

-- Any remaining membership-less user whose home workspace already
-- has an owner (e.g. two users backfilled into the shared default
-- workspace) gets an 'admin' membership instead — they keep access
-- without violating the one-owner index.
INSERT INTO workspace_members (workspace_id, user_id, role, added_by)
SELECT u.workspace_id, u.id, 'admin', u.id
FROM users u
WHERE NOT EXISTS (
    SELECT 1 FROM workspace_members m WHERE m.user_id = u.id
)
ON CONFLICT (workspace_id, user_id) DO NOTHING;

-- Switcher lookup path: "list every workspace this user belongs
-- to" queries `workspace_members` by `user_id`. The table's PK is
-- (workspace_id, user_id) — leading on workspace_id — so a
-- user_id-first index is needed for the reverse lookup.
CREATE INDEX IF NOT EXISTS workspace_members_user_idx
    ON workspace_members (user_id);
