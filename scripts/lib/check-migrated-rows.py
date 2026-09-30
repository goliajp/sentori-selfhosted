#!/usr/bin/env python3
"""What the new migrations did to the seeded rows.

    check-migrated-rows.py <postgres-url>

Reaching the end of the migrations is not the same as having migrated
anything. A migration that drops a constraint and never re-adds it, or
rewrites zero rows because its WHERE clause is wrong, exits 0 just as
happily as a correct one. Each assertion below names a migration and
the thing that would be silently wrong without it.

Two forms of question are asked, because they fail differently:
counting rows catches a rewrite that did not happen, and attempting a
write catches a constraint that is gone or still says the old thing.
"""

import subprocess
import sys

URL = sys.argv[1]


def q(sql):
    out = subprocess.run(
        ["psql", URL, "-tAq", "-c", sql],
        capture_output=True, text=True,
    )
    if out.returncode != 0:
        fail(f"query failed: {sql}\n  {out.stderr.strip()}")
    return out.stdout.strip()


def write_is_rejected(sql):
    """True when Postgres refuses the write. Rolled back either way."""
    out = subprocess.run(
        ["psql", URL, "-tAq", "-v", "ON_ERROR_STOP=1",
         "-c", f"BEGIN; {sql}; ROLLBACK;"],
        capture_output=True, text=True,
    )
    return out.returncode != 0


problems = []


def fail(msg):
    problems.append(msg)


# 0018 — the rewrite itself.
left = q("SELECT count(*) FROM event_attachments WHERE source = 'js'")
if left != "0":
    fail(f"0018 left {left} attachment(s) still spelled 'js'")

sources = q("SELECT string_agg(source, ',' ORDER BY source) FROM event_attachments")
if sources != "android,ios,javascript":
    fail(f"0018 should leave android,ios,javascript — found {sources!r}")

# 0018 — and the constraint it re-added. A dropped-and-forgotten
# constraint passes every count above.
if not write_is_rejected(
    "INSERT INTO event_attachments (ref, project_id, event_id, kind, media_type,"
    " size_bytes, blob_hash, source, captured_at) VALUES"
    " ('77777777-7777-7777-7777-777777777771',"
    " '22222222-2222-2222-2222-222222222222', '55555555-5555-5555-5555-555555555551',"
    " 'replay', 'application/json', 1, 'h', 'js', now())"
):
    fail("0018 accepts source = 'js' on insert — the CHECK is gone or unchanged")

if write_is_rejected(
    "INSERT INTO event_attachments (ref, project_id, event_id, kind, media_type,"
    " size_bytes, blob_hash, source, captured_at) VALUES"
    " ('77777777-7777-7777-7777-777777777772',"
    " '22222222-2222-2222-2222-222222222222', '55555555-5555-5555-5555-555555555551',"
    " 'replay', 'application/json', 1, 'h', 'web', now())"
):
    fail("0018 rejects source = 'web', which v4 SDKs send")

# 0018 — the same widening on events.
if write_is_rejected(
    "INSERT INTO events (id, project_id, issue_id, kind, platform, occurred_at)"
    " VALUES ('77777777-7777-7777-7777-777777777773',"
    " '22222222-2222-2222-2222-222222222222', '44444444-4444-4444-4444-444444444444',"
    " 'error', 'weapp', now())"
):
    fail("0018 rejects events.platform = 'weapp'")

kept = q("SELECT count(*) FROM events")
if kept != "3":
    fail(f"the 3 seeded events became {kept}")

# 0019 — sessions, against a project that already existed.
if write_is_rejected(
    "INSERT INTO sessions (id, project_id, status, release, environment, platform,"
    " started_at, duration_ms) VALUES"
    " ('77777777-7777-7777-7777-777777777774',"
    " '22222222-2222-2222-2222-222222222222', 'ok', '1.0.0', 'production', 'web',"
    " now(), 1000)"
):
    fail("0019 will not take a session for a pre-existing project")

# 0020 — platform becomes optional, and old rows keep their label.
if write_is_rejected(
    "INSERT INTO projects (id, name) VALUES"
    " ('77777777-7777-7777-7777-777777777775', 'no platform')"
):
    fail("0020 still requires projects.platform")

seeded_platform = q(
    "SELECT coalesce(platform, '<null>') FROM projects"
    " WHERE id = '22222222-2222-2222-2222-222222222222'"
)
if seeded_platform != "react-native":
    fail(f"0020 changed an existing project's platform to {seeded_platform!r};"
         " it says existing rows are left alone")

# 0021 — the column the webhook notifier needs.
if write_is_rejected(
    "UPDATE projects SET webhook_url = 'https://example.test/hook'"
    " WHERE id = '22222222-2222-2222-2222-222222222222'"
):
    fail("0021 did not give projects a writable webhook_url")

if problems:
    for p in problems:
        print(f"  ✗ {p}", file=sys.stderr)
    sys.exit(1)

print("      rows migrated, constraints re-added, old labels kept")
