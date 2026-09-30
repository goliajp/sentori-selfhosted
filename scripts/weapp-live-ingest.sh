#!/usr/bin/env bash
# The mini-program SDK against a real Sentori server.
#
#   bash scripts/weapp-live-ingest.sh
#
# There is no WeChat runtime outside WeChat, so `wx` is a shim: real
# HTTP for `request`, a real store for storage, and the five `on*`
# hooks called by hand. Everything above that is the code that ships.
#
# What this proves: the wire, the queueing, the storage path, the
# handler wiring, and that identity hashes without `crypto.subtle`.
# What it does not prove: that the SDK runs inside WeChat's own engine,
# which needs a phone and is written down rather than implied.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

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

PORT="$(free_port 8401)"
PGPORT="$(free_port 55455)"
PG_CONTAINER="sentori-weapplive-pg-$$"
BASE="http://127.0.0.1:${PORT}"
LOG="$(mktemp)"
SERVER_PID=""
STARTED_BREW_PG=""

cleanup() {
    [ -n "$SERVER_PID" ] && kill "$SERVER_PID" 2>/dev/null || true
    docker rm -f "$PG_CONTAINER" >/dev/null 2>&1 || true
    [ -n "$STARTED_BREW_PG" ] && brew services stop "$STARTED_BREW_PG" >/dev/null 2>&1 || true
}
trap cleanup EXIT

echo "→ postgres"
DBNAME=sentori_weapplive
# shellcheck source=scripts/lib/local-postgres.sh
. "${ROOT}/scripts/lib/local-postgres.sh"
start_local_postgres

echo "→ server"
(cd self-hosted/server && cargo build --quiet)
SENTORI_BIND="127.0.0.1:${PORT}" SENTORI_DATABASE_URL="$DB" \
    SENTORI_OWNER_EMAIL=weapp@example.com SENTORI_OWNER_PASSWORD=weapp-password-long-enough \
    SENTORI_BASE_URL="$BASE" \
    self-hosted/server/target/debug/sentori-server > "$LOG" 2>&1 &
SERVER_PID=$!
for _ in $(seq 1 60); do curl -fsS "${BASE}/healthz" >/dev/null 2>&1 && break; sleep 1; done
curl -fsS "${BASE}/healthz" >/dev/null || { echo "server never came up:"; tail -20 "$LOG"; exit 1; }

echo "→ project + ingest token"
JAR="$(mktemp)"
curl -fsS -c "$JAR" -X POST "${BASE}/auth/login" -H 'content-type: application/json' \
    -d '{"email":"weapp@example.com","password":"weapp-password-long-enough"}' >/dev/null
PROJECT="$(curl -fsS -b "$JAR" -X POST "${BASE}/admin/api/projects" \
    -H 'content-type: application/json' -d '{"name":"weapp-live","platform":"weapp"}' \
    | python3 -c 'import sys,json;print(json.load(sys.stdin)["id"])')"
TOKEN="$(curl -fsS -b "$JAR" -X POST "${BASE}/admin/api/projects/${PROJECT}/tokens" \
    -H 'content-type: application/json' -d '{"name":"weapp","scope":"ingest"}' \
    | python3 -c 'import sys,json;print(json.load(sys.stdin)["token"])')"

echo "→ build, then drive it through a wx shim"
bun run --cwd sdk/core build > /dev/null
bun run --cwd sdk/weapp build > /dev/null
OUT_FILE="$(mktemp)"
node scripts/lib/weapp-live-driver.mjs "$BASE" "$TOKEN" "$OUT_FILE"

echo "→ and the server has it"
python3 "$ROOT/scripts/lib/check-weapp-ingest.py" "$BASE" "$JAR" "$PROJECT"

echo "→ what a mini program pays for the package"
bash scripts/check-weapp-size.sh
