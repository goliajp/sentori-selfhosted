#!/usr/bin/env bash
#
# Phase 16 sub-C: Sentori PG restore from Cloudflare R2.
#
# Usage:
#   ./restore.sh                # restore latest
#   ./restore.sh 20260509-040000  # restore a specific stamp
#
# DESTRUCTIVE — drops + recreates $PG_DB. The script prompts for
# explicit "yes" confirmation. Run on a fresh VM you don't mind
# rebuilding.
#
# Required env (same as backup.sh):
#   PGPASSWORD, RCLONE_REMOTE
# Optional: PG_HOST/PG_PORT/PG_USER/PG_DB

set -euo pipefail

PG_HOST="${PG_HOST:-localhost}"
PG_PORT="${PG_PORT:-5432}"
PG_USER="${PG_USER:-sentori}"
PG_DB="${PG_DB:-sentori}"
RCLONE_REMOTE="${RCLONE_REMOTE:?required, e.g. r2:sentori-backups}"

: "${PGPASSWORD:?PGPASSWORD or ~/.pgpass required}"

log() { echo "[$(date -u +%FT%TZ)] $*"; }

STAMP="${1:-}"
if [ -z "$STAMP" ]; then
    STAMP=$(rclone lsf "$RCLONE_REMOTE/daily/" --include "sentori-*.dump" \
            | sort | tail -1 | sed 's/^sentori-//; s/\.dump$//')
    [ -n "$STAMP" ] || { echo "no dumps found in $RCLONE_REMOTE/daily/" >&2; exit 1; }
    log "no stamp given; using latest: $STAMP"
fi

DUMP_FILE="/tmp/sentori-restore-${STAMP}.dump"
log "downloading sentori-${STAMP}.dump → $DUMP_FILE"
rclone copyto "$RCLONE_REMOTE/daily/sentori-${STAMP}.dump" "$DUMP_FILE"

echo
echo "About to:"
echo "  1. DROP DATABASE \"$PG_DB\" on $PG_HOST:$PG_PORT (as $PG_USER)"
echo "  2. CREATE DATABASE \"$PG_DB\""
echo "  3. pg_restore from $DUMP_FILE"
echo
read -r -p "Type 'yes' to continue: " ans
[ "$ans" = "yes" ] || { echo "aborted"; rm -f "$DUMP_FILE"; exit 1; }

log "dropping and recreating $PG_DB"
psql -h "$PG_HOST" -p "$PG_PORT" -U "$PG_USER" -d postgres \
    -c "DROP DATABASE IF EXISTS \"$PG_DB\""
psql -h "$PG_HOST" -p "$PG_PORT" -U "$PG_USER" -d postgres \
    -c "CREATE DATABASE \"$PG_DB\""

log "restoring from $DUMP_FILE"
pg_restore \
    -h "$PG_HOST" -p "$PG_PORT" -U "$PG_USER" -d "$PG_DB" \
    --no-owner --no-acl --jobs=4 \
    "$DUMP_FILE"

rm -f "$DUMP_FILE"
log "restore complete"
