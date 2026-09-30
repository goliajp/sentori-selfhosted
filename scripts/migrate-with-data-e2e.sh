#!/usr/bin/env bash
# Migrations, against a database that already has rows in it.
#
#   bash scripts/migrate-with-data-e2e.sh
#
# Every test of the schema until now started from an empty database, so
# every migration was only ever asked what it does to nothing. Most of
# the interesting ones are about existing rows.
#
# Migration 18 rewrote `event_attachments.source` from `js` to
# `javascript` *before* dropping the constraint that still said `js`.
# On an empty database `WHERE source = 'js'` matches nothing, no row is
# written, and no constraint is checked — so it passed everywhere and
# failed on the production deploy of 4.0.0, which is the only database
# that had rows.
#
# This applies the schema in two halves: everything up to the last
# release's migrations, then a load of rows in the shapes that were
# legal at that point, then the rest. A migration that cannot survive
# data fails here instead of at 06:20 on a deploy.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# The migration this fixture's data is written against. Rows are
# inserted after it and before everything newer, so "newer" is what
# gets tested against real values.
: "${SEED_AFTER:=0017}"

free_port() {
    python3 - "$1" <<'PORTPY'
import socket, sys
for candidate in [int(sys.argv[1]), 0]:
    s = socket.socket()
    try:
        s.bind(("127.0.0.1", candidate)); print(s.getsockname()[1]); s.close(); break
    except OSError:
        s.close()
PORTPY
}

PGPORT="$(free_port 55463)"
PG_CONTAINER="sentori-migdata-pg-$$"
STARTED_BREW_PG=""

cleanup() {
    docker rm -f "$PG_CONTAINER" >/dev/null 2>&1 || true
    [ -n "$STARTED_BREW_PG" ] && brew services stop "$STARTED_BREW_PG" >/dev/null 2>&1 || true
}
trap cleanup EXIT

echo "→ postgres"
DBNAME=sentori_migdata
# shellcheck source=scripts/lib/local-postgres.sh
. "${ROOT}/scripts/lib/local-postgres.sh"
start_local_postgres

psql_() { psql "$DB" -v ON_ERROR_STOP=1 -q "$@"; }

echo "→ schema up to ${SEED_AFTER}"
APPLIED=0
for f in core/migrations/*.sql; do
    n="$(basename "$f" | cut -d_ -f1)"
    [ "$n" \> "$SEED_AFTER" ] && continue
    psql_ -f "$f" || { echo "✗ ${f} failed on an empty database" >&2; exit 1; }
    APPLIED=$((APPLIED + 1))
done
echo "      ${APPLIED} migrations"

echo "→ rows, in the shapes that were legal there"
psql_ -f scripts/lib/migration-seed.sql \
    || { echo "✗ the seed does not fit the schema at ${SEED_AFTER} — it has drifted" >&2; exit 1; }
BEFORE="$(psql "$DB" -tAc "SELECT count(*) FROM event_attachments WHERE source = 'js'")"
[ "$BEFORE" -ge 1 ] \
    || { echo "✗ the seed inserted no 'js' attachment, so migration 18 has nothing to rewrite" >&2; exit 1; }
echo "      ${BEFORE} attachment(s) with the old 'js' spelling"

echo "→ the rest of the migrations, on top of those rows"
REST=0
for f in core/migrations/*.sql; do
    n="$(basename "$f" | cut -d_ -f1)"
    [ "$n" \> "$SEED_AFTER" ] || continue
    psql_ -f "$f" || {
        echo "✗ ${f} failed on a database with rows in it." >&2
        echo "  It passes on an empty one, which is why nothing caught it before." >&2
        exit 1
    }
    REST=$((REST + 1))
done
echo "      ${REST} migrations"

echo "→ and the rows were migrated, not just tolerated"
python3 "$ROOT/scripts/lib/check-migrated-rows.py" "$DB"

# Does the fixture still have teeth?
#
# Everything above passes just as well against a seed that inserts
# nothing interesting, and a seed drifts: a column is renamed, an
# INSERT quietly stops matching the shape that used to break things,
# and this script keeps saying yes to a database that can no longer
# reproduce anything. So ask Postgres directly whether the row the
# whole script is built around would have been refused by the old
# constraint — if it would not, the seed no longer reproduces the class
# of defect and the green above is decoration.
echo "→ the seed can still reproduce the defect"
REFUSED=$(psql "$DB" -tAq -v ON_ERROR_STOP=1 <<'PROBE' 2>&1 >/dev/null || true
BEGIN;
UPDATE event_attachments SET source = 'js' WHERE source = 'javascript';
ALTER TABLE event_attachments DROP CONSTRAINT event_attachments_source_check;
ALTER TABLE event_attachments ADD CONSTRAINT event_attachments_source_check
  CHECK (source IN ('js', 'ios', 'android'));
-- What migration 18 did in the order it did it.
UPDATE event_attachments SET source = 'javascript' WHERE source = 'js';
ROLLBACK;
PROBE
)
case "$REFUSED" in
    *violates\ check\ constraint*)
        echo "      yes — a rewrite under the old constraint is still refused" ;;
    *)
        echo "✗ the old constraint no longer refuses the rewrite." >&2
        echo "  This script would not have caught the 4.0.0 deploy failure." >&2
        echo "  Either the seed stopped inserting a row of that shape, or the" >&2
        echo "  table changed. psql said:" >&2
        printf '%s\n' "${REFUSED:-<nothing>}" | sed 's/^/    /' >&2
        exit 1 ;;
esac
