-- A second alert channel, because the first one needs an SMTP server.
--
-- Issue notifications have been email-only since they existed, and
-- `spawn_issue_notification` returns early when no SMTP is
-- configured. A self-hosted instance without a mail server therefore
-- gets no alerts at all, and nothing anywhere says so — the operator
-- finds out by never hearing about an outage.
--
-- `WebhookTransport` has been in the notifier crate the whole time,
-- with tests and a doc comment teaching people to use it, and the
-- server never registered it. This is the column that makes it real.
--
-- Per project, not per user: a webhook goes to a room, not to a
-- person. Everyone who can see the project sees what lands there, so
-- there is no per-user subscription to honour.
ALTER TABLE projects ADD COLUMN webhook_url TEXT;

-- The URL is a secret in the same way an ingest token is: whoever has
-- it can post into the team's channel. Nothing indexes it and nothing
-- returns it to a non-superadmin.
COMMENT ON COLUMN projects.webhook_url IS
    'Outbound alert webhook. Full URL; POSTed a {subject, body, metadata} JSON body.';
