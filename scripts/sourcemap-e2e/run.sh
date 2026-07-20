#!/usr/bin/env bash
#
# Phase 16 sub-E (回填 Phase 8): round-trip source-map e2e.
#
# Bundles the fixture, uploads the .map via sentori-cli, evaluates the
# bundle in Node so a real stack lands in the dashboard, then asserts
# the symbolicated frames point at the original source.

set -euo pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
DIST="$HERE/dist"

: "${SENTORI_BASE:=http://localhost:8080}"
: "${SENTORI_DEV_TOKEN:?required}"
: "${SENTORI_PROJECT_ID:?required}"

RELEASE="sourcemap-e2e@1.0.0+$(date +%s)"
mkdir -p "$DIST"

echo "[1/4] bundling fixture (Metro)"
(
  cd "$HERE"
  bun x metro build app.tsx \
    --out "$DIST/bundle.js" \
    --sourcemap-output "$DIST/bundle.js.map" \
    --minify true \
    --platform web \
    --dev false
)

echo "[2/4] uploading source map for release $RELEASE"
"$ROOT/cli/target/release/sentori-cli" upload sourcemap \
  --release "$RELEASE" \
  --token "$SENTORI_DEV_TOKEN" \
  --ingest-url "$SENTORI_BASE" \
  "$DIST/bundle.js.map"

echo "[3/4] evaluating bundle in Node, capturing the throw"
EVENT_JSON=$(node "$HERE/throw-and-format.js" "$DIST/bundle.js" "$RELEASE")
EVENT_ID=$(echo "$EVENT_JSON" | python3 -c 'import sys,json; print(json.load(sys.stdin)["id"])')

# POST it to ingest with the dev token.
curl -sS -o /dev/null -w 'ingest=%{http_code}\n' -X POST "$SENTORI_BASE/v1/events" \
  -H "Authorization: Bearer $SENTORI_DEV_TOKEN" \
  -H 'Content-Type: application/json' \
  --data-raw "$EVENT_JSON"

# Look the issue back up via the admin API.
sleep 1
ISSUES=$(curl -sS "$SENTORI_BASE/admin/api/projects/$SENTORI_PROJECT_ID/issues?limit=50" \
  -H "Authorization: Bearer $SENTORI_DEV_TOKEN")
ISSUE_ID=$(echo "$ISSUES" | python3 -c '
import sys, json
issues = json.load(sys.stdin)
boom = [i for i in issues if i["lastRelease"] == "'"$RELEASE"'"]
print(boom[0]["id"] if boom else "")
')
[ -n "$ISSUE_ID" ] || { echo "FAIL no issue found for release $RELEASE"; exit 1; }

echo "[4/4] fetching event with symbolicated=true; checking frames"
EVENTS=$(curl -sS \
  "$SENTORI_BASE/admin/api/projects/$SENTORI_PROJECT_ID/issues/$ISSUE_ID/events?symbolicated=true&limit=1" \
  -H "Authorization: Bearer $SENTORI_DEV_TOKEN")

# The top of the symbolicated stack should reference app.tsx, not bundle.js.
TOP_FRAME=$(echo "$EVENTS" | python3 -c '
import sys, json
events = json.load(sys.stdin)
frame = events[0]["payload"]["error"]["stack"][0]
print(frame.get("file", ""), frame.get("function", ""), frame.get("line", ""))
')
echo "  symbolicated top frame: $TOP_FRAME"

echo "$TOP_FRAME" | grep -q "app.tsx" \
  || { echo "FAIL top frame still points at bundle.js after symbolication: $TOP_FRAME" >&2; exit 1; }

echo
echo "Source-map e2e: PASSED"
