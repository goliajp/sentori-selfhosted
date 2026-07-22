-- Sentori core migration 0025 — issue dashboard tables
-- (issue_comments + watchers + notifications + activity_log +
-- issue_user_mutes).
--
-- These power the dashboard issue-detail interactive surfaces:
--   • issue_comments    — per-issue threaded discussion
--   • watchers          — who gets notified on issue mutation
--   • notifications     — per-user in-app inbox row
--   • activity_log      — append-only mutation history per issue
--   • issue_user_mutes  — per-user mute (silences inbox + email)
--
-- Columns verbatim from legacy:
--   issue_comments      — 0019_issue_comments.sql
--   watchers + notifications — 0052_watchers_notifications.sql
--   activity_log        — 0049_activity_log.sql
--   issue_user_mutes    — 0060_issue_user_mutes.sql

CREATE TABLE IF NOT EXISTS issue_comments (
    id         UUID        PRIMARY KEY,
    issue_id   UUID        NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    author_id  UUID        REFERENCES users(id) ON DELETE SET NULL,
    body       TEXT        NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS issue_comments_issue_idx
    ON issue_comments (issue_id, created_at);

CREATE TABLE IF NOT EXISTS watchers (
    issue_id UUID        NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    user_id  UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    since    TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (issue_id, user_id)
);

CREATE INDEX IF NOT EXISTS watchers_user_idx
    ON watchers (user_id);

CREATE TABLE IF NOT EXISTS notifications (
    id         BIGSERIAL   PRIMARY KEY,
    user_id    UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    issue_id   UUID        NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    kind       TEXT        NOT NULL,
    payload    JSONB       NOT NULL DEFAULT '{}'::jsonb,
    read_at    TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS notifications_user_unread_idx
    ON notifications (user_id, created_at DESC)
    WHERE read_at IS NULL;
CREATE INDEX IF NOT EXISTS notifications_user_recent_idx
    ON notifications (user_id, created_at DESC);

CREATE TABLE IF NOT EXISTS activity_log (
    id        BIGSERIAL   PRIMARY KEY,
    issue_id  UUID        NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    actor_id  UUID        REFERENCES users(id) ON DELETE SET NULL,
    verb      TEXT        NOT NULL,
    payload   JSONB       NOT NULL DEFAULT '{}'::jsonb,
    at        TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS activity_log_issue_at_idx
    ON activity_log (issue_id, at DESC);
CREATE INDEX IF NOT EXISTS activity_log_actor_at_idx
    ON activity_log (actor_id, at DESC)
    WHERE actor_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS issue_user_mutes (
    user_id  UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    issue_id UUID        NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    since    TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, issue_id)
);

CREATE INDEX IF NOT EXISTS issue_user_mutes_user_idx
    ON issue_user_mutes (user_id);
