-- Sessions, so the product can answer "what is our crash-free rate".
--
-- It could not. The five kinds count what went wrong and nothing
-- counts what went right, so "18 errors" has no denominator and the
-- first number a mobile team is asked for — the share of sessions
-- that did not crash — was unanswerable. Two separate reviewers named
-- it as the reason not to adopt.
--
-- A row here is a fact: a session happened, it lasted this long, and
-- it ended this way. Append-only, never updated. The rate is a
-- derivation computed on read, so deleting every cached number and
-- recomputing gives the same answer.
--
-- `id` is minted by the SDK, and a resent ping lands on the same one:
-- a lost response must not turn one session into two, which is the
-- same lesson the events table learned as a primary-key violation
-- dressed up as a server fault.

CREATE TABLE sessions (
    id          uuid        PRIMARY KEY,
    project_id  uuid        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    -- Monotonic on the client: once crashed, a later error cannot
    -- demote it. The server stores what arrives and does not re-rank.
    status      text        NOT NULL
                            CHECK (status IN ('ok', 'exited', 'errored', 'crashed')),
    release     text        NOT NULL,
    environment text        NOT NULL,
    platform    text        NOT NULL
                            CHECK (platform IN ('javascript', 'ios', 'android',
                                                'web', 'weapp', 'unknown')),
    -- When the session began on the device, and when we heard about
    -- it. Both, for the same reason events carry both: a device clock
    -- is not a fact about our system.
    started_at  timestamptz NOT NULL,
    received_at timestamptz NOT NULL DEFAULT now(),
    duration_ms integer     NOT NULL CHECK (duration_ms >= 0),
    -- The same salted hash events carry, so crash-free *users* and
    -- crash-free *sessions* come from one identity.
    user_key    text
);

-- The aggregate is always "this project, this window", and usually
-- also "this release" — the comparison a release decision needs.
CREATE INDEX idx_sessions_project_started ON sessions (project_id, started_at DESC);
CREATE INDEX idx_sessions_project_release ON sessions (project_id, release, environment);
