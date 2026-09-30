#!/usr/bin/env bash
# The browser SDK in a real Chrome, against a real Sentori server.
#
#   bash scripts/web-live-ingest.sh
#
# Everything this SDK does that matters happens against APIs Bun does
# not have — `addEventListener('error')`, `PerformanceObserver`,
# `localStorage`, `visibilitychange`. Unit tests of those would be
# tests of whatever stand-in was written for them, so the gate is a
# page in a browser talking to the server.
#
# It also measures the long-task budget in the same run. A reporter
# that costs the page a frame is one the host removes, so "no long
# task while the SDK does 550 calls" is a claim that has to be checked
# rather than asserted in a comment.

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

PORT="$(free_port 8398)"
PGPORT="$(free_port 55443)"
WEBPORT="$(free_port 8399)"
PG_CONTAINER="sentori-weblive-pg-$$"
BASE="http://127.0.0.1:${PORT}"
LOG="$(mktemp)"
SERVE_DIR="$(mktemp -d)"
SERVER_PID=""
WEB_PID=""
STARTED_BREW_PG=""

cleanup() {
    [ -n "$SERVER_PID" ] && kill "$SERVER_PID" 2>/dev/null || true
    [ -n "$WEB_PID" ] && kill "$WEB_PID" 2>/dev/null || true
    docker rm -f "$PG_CONTAINER" >/dev/null 2>&1 || true
    [ -n "$STARTED_BREW_PG" ] && brew services stop "$STARTED_BREW_PG" >/dev/null 2>&1 || true
    rm -rf "$SERVE_DIR"
}
trap cleanup EXIT

echo "→ postgres"
DBNAME=sentori_weblive
# shellcheck source=scripts/lib/local-postgres.sh
. "${ROOT}/scripts/lib/local-postgres.sh"
start_local_postgres

echo "→ server"
(cd self-hosted/server && cargo build --quiet)
SENTORI_BIND="127.0.0.1:${PORT}" SENTORI_DATABASE_URL="$DB" \
    SENTORI_OWNER_EMAIL=web@example.com SENTORI_OWNER_PASSWORD=web-password-long-enough \
    SENTORI_BASE_URL="$BASE" \
    self-hosted/server/target/debug/sentori-server > "$LOG" 2>&1 &
SERVER_PID=$!
for _ in $(seq 1 60); do curl -fsS "${BASE}/healthz" >/dev/null 2>&1 && break; sleep 1; done
curl -fsS "${BASE}/healthz" >/dev/null || { echo "server never came up:"; tail -20 "$LOG"; exit 1; }

echo "→ project + ingest token"
JAR="$(mktemp)"
curl -fsS -c "$JAR" -X POST "${BASE}/auth/login" -H 'content-type: application/json' \
    -d '{"email":"web@example.com","password":"web-password-long-enough"}' >/dev/null
PROJECT="$(curl -fsS -b "$JAR" -X POST "${BASE}/admin/api/projects" \
    -H 'content-type: application/json' -d '{"name":"web-live","platform":"web"}' \
    | python3 -c 'import sys,json;print(json.load(sys.stdin)["id"])')"
TOKEN="$(curl -fsS -b "$JAR" -X POST "${BASE}/admin/api/projects/${PROJECT}/tokens" \
    -H 'content-type: application/json' -d '{"name":"web","scope":"ingest"}' \
    | python3 -c 'import sys,json;print(json.load(sys.stdin)["token"])')"

echo "→ bundle the SDK the way a site would"
bun run --cwd sdk/core build > /dev/null
bun run --cwd sdk/web build > /dev/null
# One file, the way a `<script type="module">` gets it. Bundled rather
# than served as loose ESM so this exercises what an integrator ships,
# and so the size the next step measures is the size they pay.
bunx esbuild sdk/web/lib/index.js --bundle --format=esm --minify \
    --outfile="${SERVE_DIR}/sentori-web.js" --log-level=error
cp sdk/web/fixtures/page.html "${SERVE_DIR}/index.html"

# The page must be served over http: a module script from file:// is
# blocked, and `localStorage` on an opaque origin throws.
(cd "$SERVE_DIR" && python3 -m http.server "$WEBPORT" >/dev/null 2>&1) &
WEB_PID=$!
for _ in $(seq 1 40); do curl -fsS "http://127.0.0.1:${WEBPORT}/index.html" >/dev/null 2>&1 && break; sleep 0.25; done

echo "→ a real browser"
OUT_FILE="$(mktemp)"
node scripts/lib/web-live-driver.mjs \
    "http://127.0.0.1:${WEBPORT}/index.html?token=${TOKEN}&ingest=${BASE}&replay=1" "$OUT_FILE"

echo "→ and the server has it"
python3 "$ROOT/scripts/lib/check-web-ingest.py" "$BASE" "$JAR" "$PROJECT" "$OUT_FILE"

echo "→ what a site pays for the bundle"
bash scripts/check-web-size.sh "${SERVE_DIR}/sentori-web.js"
