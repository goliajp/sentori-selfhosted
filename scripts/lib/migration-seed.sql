-- Rows in the shapes that were legal at migration 0017.
--
-- Loaded by scripts/migrate-with-data-e2e.sh between the old
-- migrations and the new ones, so the new ones are asked what they do
-- to data rather than to an empty schema.
--
-- Two rules for anything added here:
--
--   * use the spellings and constraints of the era, not today's. The
--     point is to be a database that predates the migration under
--     test. `event_attachments.source = 'js'` is here precisely
--     because 0018 has to rewrite it.
--   * when a migration changes a column, add a row that has the old
--     value. A migration with no matching row is not tested by this
--     script, only carried past it.

INSERT INTO users (id, email, password_hash, role) VALUES
    ('11111111-1111-1111-1111-111111111111', 'seed@example.test', 'x', 'superadmin');

INSERT INTO projects (id, name, platform) VALUES
    ('22222222-2222-2222-2222-222222222222', 'seed', 'react-native');

INSERT INTO tokens (id, project_id, name, scope, token_hash, last4) VALUES
    ('33333333-3333-3333-3333-333333333333',
     '22222222-2222-2222-2222-222222222222', 'seed', 'ingest', 'seedhash', 'aaaa');

INSERT INTO issues (id, project_id, fingerprint, kind, group_title,
                    first_seen, last_seen, event_count, platform, environment) VALUES
    ('44444444-4444-4444-4444-444444444444',
     '22222222-2222-2222-2222-222222222222', 'fp-seed', 'error', 'Seed error',
     now() - interval '2 days', now(), 3, 'javascript', 'production');

-- One event per platform the old CHECK allowed.
INSERT INTO events (id, project_id, issue_id, kind, platform, occurred_at, release, environment, user_key) VALUES
    ('55555555-5555-5555-5555-555555555551',
     '22222222-2222-2222-2222-222222222222', '44444444-4444-4444-4444-444444444444',
     'error', 'javascript', now() - interval '2 days', '1.0.0', 'production', 'uk-a'),
    ('55555555-5555-5555-5555-555555555552',
     '22222222-2222-2222-2222-222222222222', '44444444-4444-4444-4444-444444444444',
     'error', 'ios', now() - interval '1 day', '1.0.0', 'production', 'uk-b'),
    ('55555555-5555-5555-5555-555555555553',
     '22222222-2222-2222-2222-222222222222', '44444444-4444-4444-4444-444444444444',
     'error', 'android', now(), '1.0.0', 'production', 'uk-c');

-- And one attachment per spelling the old CHECK allowed, including the
-- `js` that 0018 has to rewrite.
INSERT INTO event_attachments (ref, project_id, event_id, kind, media_type,
                               size_bytes, blob_hash, source, captured_at) VALUES
    ('66666666-6666-6666-6666-666666666661',
     '22222222-2222-2222-2222-222222222222', '55555555-5555-5555-5555-555555555551',
     'replay', 'application/json', 10, 'h1', 'js', now()),
    ('66666666-6666-6666-6666-666666666662',
     '22222222-2222-2222-2222-222222222222', '55555555-5555-5555-5555-555555555552',
     'screenshot', 'image/png', 20, 'h2', 'ios', now()),
    ('66666666-6666-6666-6666-666666666663',
     '22222222-2222-2222-2222-222222222222', '55555555-5555-5555-5555-555555555553',
     'viewTree', 'application/json', 30, 'h3', 'android', now());
