#!/usr/bin/env bash
# Phase 4 end-to-end smoke test.
#
# Boots sentori-server and uses the @sentori/react-native SDK transport
# (in bun) to send one event, then verifies it lands in /v1/events/_recent.
#
# Does NOT require iOS simulator or Android emulator. The simulator path
# is exercised manually — see sdk/react-native/example/README.md.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
HERE="$ROOT/e2e"
TOKEN="st_pk_dev0000000000000000000000"
PORT="${SENTORI_PORT:-8080}"
URL="http://127.0.0.1:${PORT}"

SERVER_PID=""
cleanup() {
  if [ -n "$SERVER_PID" ]; then
    kill "$SERVER_PID" 2>/dev/null || true
    wait "$SERVER_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

echo "==> Installing e2e deps..."
cd "$HERE"
bun install --silent

echo "==> Ensuring SDK is built..."
cd "$ROOT/sdk/react-native"
if [ ! -f lib/index.js ]; then
  bun install --silent
  bun run build
fi

echo "==> Building server..."
cd "$ROOT/server"
cargo build --bin sentori-server --quiet

echo "==> Starting server on $URL..."
SENTORI_DEV_TOKEN="$TOKEN" "$ROOT/server/target/debug/sentori-server" >/tmp/sentori-server.log 2>&1 &
SERVER_PID=$!

# wait for ready
for _ in $(seq 1 60); do
  if curl -fsS "$URL/v1/events/_recent" -H "Authorization: Bearer $TOKEN" >/dev/null 2>&1; then
    echo "==> Server up (pid $SERVER_PID)"
    break
  fi
  sleep 0.3
done

if ! curl -fsS "$URL/v1/events/_recent" -H "Authorization: Bearer $TOKEN" >/dev/null 2>&1; then
  echo "FAIL: server did not start within 18s"
  cat /tmp/sentori-server.log
  exit 1
fi

echo "==> Sending event via SDK transport..."
cd "$HERE"
INGEST_URL="$URL" SENTORI_TOKEN="$TOKEN" bun send-event.ts

echo "==> Verifying via /v1/events/_recent..."
sleep 1
RESP=$(curl -fsS "$URL/v1/events/_recent" -H "Authorization: Bearer $TOKEN")
COUNT=$(echo "$RESP" | python3 -c "import sys, json; print(len(json.load(sys.stdin)))")

if [ "$COUNT" -lt 1 ]; then
  echo "FAIL: expected >= 1 event in _recent, got $COUNT"
  echo "Response: $RESP"
  echo "Server log:"
  cat /tmp/sentori-server.log
  exit 1
fi

FIRST_PLATFORM=$(echo "$RESP" | python3 -c "import sys, json; print(json.load(sys.stdin)[0]['platform'])")
FIRST_TYPE=$(echo "$RESP" | python3 -c "import sys, json; print(json.load(sys.stdin)[0]['error']['type'])")

echo "==> PASS"
echo "    events: $COUNT"
echo "    platform: $FIRST_PLATFORM"
echo "    error.type: $FIRST_TYPE"
