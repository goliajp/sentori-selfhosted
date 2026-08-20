#!/usr/bin/env bash
#
# Phase 16 sub-C: Sentori PG nightly backup → Cloudflare R2.
#
# Run from cron on the PG VM, e.g.:
#   0 4 * * * /opt/sentori/backup.sh >> /var/log/sentori-backup.log 2>&1
#
# Required env (set in the cron line, /etc/sentori.env, or PG VM's
# system service):
#   PGPASSWORD          (or use ~/.pgpass)
#   RCLONE_REMOTE       e.g. r2:sentori-backups
# Optional:
#   PG_HOST/PG_PORT/PG_USER/PG_DB    default localhost / 5432 / sentori / sentori
#   RETENTION_DAYS      default 30
#
# Restore counterpart: ops/restore.sh
# WAL archiving for PITR: ops/postgresql.archive.conf

set -euo pipefail

PG_HOST="${PG_HOST:-localhost}"
PG_PORT="${PG_PORT:-5432}"
PG_USER="${PG_USER:-sentori}"
PG_DB="${PG_DB:-sentori}"
RCLONE_REMOTE="${RCLONE_REMOTE:?required, e.g. r2:sentori-backups}"
RETENTION_DAYS="${RETENTION_DAYS:-30}"

: "${PGPASSWORD:?PGPASSWORD or ~/.pgpass required}"

STAMP=$(date -u +%Y%m%d-%H%M%S)
DUMP_FILE="/tmp/sentori-${STAMP}.dump"

log() { echo "[$(date -u +%FT%TZ)] $*"; }

log "starting pg_dump → $DUMP_FILE"
pg_dump \
    -h "$PG_HOST" -p "$PG_PORT" -U "$PG_USER" \
    --format=custom --no-owner --no-acl --jobs=1 \
    -f "$DUMP_FILE" "$PG_DB"

SIZE=$(stat -f%z "$DUMP_FILE" 2>/dev/null || stat -c%s "$DUMP_FILE")
log "dump complete (${SIZE} bytes); uploading"

rclone copyto "$DUMP_FILE" "$RCLONE_REMOTE/daily/sentori-${STAMP}.dump"
log "upload complete"

# Retention sweep — drop dumps older than $RETENTION_DAYS in R2.
rclone delete --min-age "${RETENTION_DAYS}d" "$RCLONE_REMOTE/daily/" \
    --include "sentori-*.dump" || true
log "retention sweep done (>${RETENTION_DAYS}d)"

rm -f "$DUMP_FILE"
log "backup complete"
