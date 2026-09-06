-- Sentori v1 migration 0003 — the five-kind event pipeline.
--
-- The kind vocabulary IS the concept model (design.md §2): five
-- verbs, five kinds, no severity dimension. The v0.2 vocabulary
-- (error|anr|near_crash|message) maps as: fatal ANR → error,
-- recoverable freeze / near_crash → warn scenarios, message → gone
-- (trace is its native replacement).
--
-- issues carries the objective-importance pair the product ranks by
-- (design.md §2): users_count (breadth — how many distinct users)
-- and max_per_user (depth — the worst single user's hit count),
-- both maintained incrementally from issue_user_hits at ingest.
-- No severity column, deliberately: Sentry lets the developer
-- declare importance; Sentori computes it from evidence.
--
-- Status is three states + one flag, not four states: `regressed`
-- is an open issue with regressed_at set, so "open because it came
-- back" never diverges from "open". Resolve anchors on a release
-- (resolved_in_release); only a recurrence in a release >= that one
-- counts as regression (design.md §11), which is what keeps
-- old-version long-tail events from crying wolf.
--
-- events is append-only, unpartitioned: the v0.2 spans table needed
-- RANGE partitions because replay ticks flooded it (1.5M rows in 22
-- days, 97% heartbeat). B-type replay killed the flood; what is
-- left is actual signal at ~10^3/day. BRIN on received_at covers
-- retention scans. No spans table in v1 — trace is just a kind.

CREATE TABLE issues (
    id                   UUID        PRIMARY KEY,
    project_id           UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    fingerprint          TEXT        NOT NULL,
    kind                 TEXT        NOT NULL
                                     CHECK (kind IN ('error', 'warn', 'trace', 'assert', 'probe')),
    -- What the group is "about"; per kind: error → exception type,
    -- warn → scenario (detected) or name (hand-written),
    -- trace/assert → name, probe → ref.
    group_title          TEXT        NOT NULL,
    message_sample       TEXT        NOT NULL DEFAULT '',
    -- warn scenarios carry where it happened; JSONB because surface
    -- shape differs per category ({screen, element} for interaction,
    -- {screen} for loading, {} for resource).
    surface              JSONB       NOT NULL DEFAULT '{}'::jsonb,
    status               TEXT        NOT NULL DEFAULT 'open'
                                     CHECK (status IN ('open', 'resolved', 'ignored')),
    first_seen           TIMESTAMPTZ NOT NULL,
    last_seen            TIMESTAMPTZ NOT NULL,
    event_count          BIGINT      NOT NULL DEFAULT 0,
    users_count          BIGINT      NOT NULL DEFAULT 0,
    max_per_user         BIGINT      NOT NULL DEFAULT 0,
    last_environment     TEXT        NOT NULL DEFAULT '',
    last_release         TEXT        NOT NULL DEFAULT '',
    assignee_user_id     UUID        REFERENCES users(id) ON DELETE SET NULL,
    resolved_at          TIMESTAMPTZ,
    resolved_in_release  TEXT,
    regressed_at         TIMESTAMPTZ,
    regressed_in_release TEXT,
    UNIQUE (project_id, fingerprint)
);
CREATE INDEX issues_project_last_seen_idx
    ON issues (project_id, last_seen DESC);
CREATE INDEX issues_project_status_idx
    ON issues (project_id, status);
CREATE INDEX issues_assignee_idx
    ON issues (assignee_user_id) WHERE assignee_user_id IS NOT NULL;

CREATE TABLE events (
    id          UUID        PRIMARY KEY,
    project_id  UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    issue_id    UUID        NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    kind        TEXT        NOT NULL
                            CHECK (kind IN ('error', 'warn', 'trace', 'assert', 'probe')),
    platform    TEXT        NOT NULL CHECK (platform IN ('javascript', 'ios', 'android')),
    -- Device clock vs server clock, kept apart on purpose: mobile
    -- clocks lie, and retention/ordering must use ours.
    occurred_at TIMESTAMPTZ NOT NULL,
    received_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    release     TEXT        NOT NULL DEFAULT '',
    environment TEXT        NOT NULL DEFAULT '',
    -- Salted identity hash (identity-fingerprint crate); drives the
    -- breadth × depth stats without storing raw user ids.
    user_key    TEXT,
    payload     JSONB       NOT NULL DEFAULT '{}'::jsonb
);
CREATE INDEX events_project_received_idx
    ON events (project_id, received_at DESC);
CREATE INDEX events_issue_received_idx
    ON events (issue_id, received_at DESC);
CREATE INDEX events_received_brin_idx
    ON events USING BRIN (received_at);

-- One row per (issue, user); the incremental substrate for breadth
-- (COUNT(*)) and depth (MAX(hit_count)) — both denormalized onto
-- issues at ingest so the Inbox never aggregates this table.
CREATE TABLE issue_user_hits (
    issue_id  UUID        NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    user_key  TEXT        NOT NULL,
    hit_count BIGINT      NOT NULL DEFAULT 1,
    last_hit  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (issue_id, user_key)
);

-- Append-only activity: status flips, assignments, regressions, and
-- notes from humans or AI agents. Not a comment system — no threads,
-- no mentions. It exists so an agent can write back "fixed in
-- abc123, probe planted" and resolve (design.md §9).
CREATE TABLE issue_activity (
    id            UUID        PRIMARY KEY,
    issue_id      UUID        NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- NULL actor = the system itself (regression detection, ingest).
    actor_user_id UUID        REFERENCES users(id) ON DELETE SET NULL,
    kind          TEXT        NOT NULL
                              CHECK (kind IN ('status', 'assign', 'note', 'regression')),
    body          JSONB       NOT NULL DEFAULT '{}'::jsonb
);
CREATE INDEX issue_activity_issue_idx
    ON issue_activity (issue_id, at DESC);
