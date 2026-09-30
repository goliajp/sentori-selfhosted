#!/usr/bin/env bash
# The extracted kernel against a real Sentori server.
#
#   bash scripts/core-live-ingest.sh
#
# `sdk/core`'s transport is what the web and mini-program SDKs will be
# built on, and every test of it uses a fake `fetch` — which agrees
# with whatever mistake the envelope is making. The iOS and Android
# SDKs each got a live-ingest gate for exactly this reason; the kernel
# they now share had none.
#
# It also drives the offline path, which had never run against a real
# endpoint: the batch that fails, the storage it lands in, and the
# next launch that delivers it.

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

PORT="$(free_port 8397)"
PGPORT="$(free_port 55441)"
PG_CONTAINER="sentori-corelive-pg-$$"
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
DBNAME=sentori_corelive
# shellcheck source=scripts/lib/local-postgres.sh
. "${ROOT}/scripts/lib/local-postgres.sh"
start_local_postgres

echo "→ server"
(cd self-hosted/server && cargo build --quiet)
SENTORI_BIND="127.0.0.1:${PORT}" SENTORI_DATABASE_URL="$DB" \
    SENTORI_OWNER_EMAIL=core@example.com SENTORI_OWNER_PASSWORD=core-password-long-enough \
    SENTORI_BASE_URL="$BASE" \
    self-hosted/server/target/debug/sentori-server > "$LOG" 2>&1 &
SERVER_PID=$!
for _ in $(seq 1 60); do curl -fsS "${BASE}/healthz" >/dev/null 2>&1 && break; sleep 1; done
curl -fsS "${BASE}/healthz" >/dev/null || { echo "server never came up:"; tail -20 "$LOG"; exit 1; }

echo "→ project + ingest token"
JAR="$(mktemp)"
curl -fsS -c "$JAR" -X POST "${BASE}/auth/login" -H 'content-type: application/json' \
    -d '{"email":"core@example.com","password":"core-password-long-enough"}' >/dev/null
PROJECT="$(curl -fsS -b "$JAR" -X POST "${BASE}/admin/api/projects" \
    -H 'content-type: application/json' -d '{"name":"core-live","platform":"web"}' \
    | python3 -c 'import sys,json;print(json.load(sys.stdin)["id"])')"
TOKEN="$(curl -fsS -b "$JAR" -X POST "${BASE}/admin/api/projects/${PROJECT}/tokens" \
    -H 'content-type: application/json' -d '{"name":"core","scope":"ingest"}' \
    | python3 -c 'import sys,json;print(json.load(sys.stdin)["token"])')"

# The driver imports the built kernel, not the sources: what ships is
# what gets driven.
bun run --cwd sdk/core build > /dev/null

echo "→ the kernel sends, fails, persists and re-sends"
IDS_FILE="$(mktemp)"
node scripts/lib/core-live-driver.mjs "$BASE" "$TOKEN" "$IDS_FILE"
IDS="$(cat "$IDS_FILE")"

echo "→ and the server has all of it"
python3 - "$BASE" "$JAR" "$PROJECT" "$IDS" <<'PY'
import json, subprocess, sys
base, jar, project, ids = sys.argv[1:5]
want = json.loads(ids)

def get(path):
    out = subprocess.run(["curl", "-fsS", "-b", jar, f"{base}{path}"],
                         capture_output=True, text=True).stdout
    return json.loads(out or "{}")

for label, event_id in (("the ordinary batch", want["directId"]),
                        ("the batch that went through storage", want["offlineId"])):
    ev = get(f"/admin/api/events/{event_id}")
    if not ev.get("id"):
        sys.exit(f"✗ {label} ({event_id}) is not on the server")
    if ev.get("platform") != "web":
        sys.exit(f"✗ {label} stored platform {ev.get('platform')!r}, want 'web'")
    if not ev.get("userKey"):
        sys.exit(f"✗ {label} arrived with no userKey")

cf = get(f"/admin/api/sessions/crash-free?projectId={project}&hours=720")
if cf.get("sessions") != 1:
    sys.exit(f"✗ the session the kernel queued did not arrive: {cf}")

issues = get(f"/admin/api/issues?projectId={project}").get("issues") or []
kinds = sorted({i["kind"] for i in issues})
if kinds != ["error", "warn"]:
    sys.exit(f"✗ server stored kinds {kinds}, want ['error', 'warn']")

print("✓ the server accepts what the extracted kernel sends, including "
      "the batch it had to store and re-send")
PY
