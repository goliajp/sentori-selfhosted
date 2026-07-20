#!/usr/bin/env bash
#
# Phase 14 SaaS-onboarding smoke. Walks the full flow a new SaaS user
# would take from sentori.golia.jp:
#   register → verify (auto-bootstrap personal org via sub-H) → login →
#   create project → create public token → POST /v1/events with that
#   token → confirm an issue lands → revoke → POST again → 401.
#
# Required env (or sensible dev defaults are used):
#   SENTORI_BASE      base URL of the running server (default http://localhost:8080)
#   PG_CONTAINER      docker container name of postgres   (default sentori-pg)
#   PG_USER / PG_DB   pg login                            (default postgres / sentori)

set -euo pipefail

BASE="${SENTORI_BASE:-http://localhost:8080}"
PG_CONTAINER="${PG_CONTAINER:-sentori-pg}"
PG_USER="${PG_USER:-postgres}"
PG_DB="${PG_DB:-sentori}"

pg() { docker exec "$PG_CONTAINER" psql -U "$PG_USER" -d "$PG_DB" -tA -c "$1" | tr -d '\r'; }

RUN_ID=$(date +%s%N)
EMAIL="grace-${RUN_ID}@test.local"
CK=$(mktemp)
trap 'rm -f "$CK"' EXIT

assert_status() {
  local label=$1 want=$2 got=$3
  if [ "$got" != "$want" ]; then
    echo "FAIL $label — expected http=$want, got $got" >&2
    exit 1
  fi
  echo "  PASS $label (http=$got)"
}

echo "Phase 14 SaaS onboarding smoke (run $RUN_ID)"
echo

echo "[1/8] register + verify + login (sub-B + sub-H bootstrap)"
curl -sS -X POST "$BASE/api/auth/register" \
  -H 'content-type: application/json' \
  -d "{\"email\":\"$EMAIL\",\"password\":\"hunter2hunter2\"}" >/dev/null
VTOKEN=$(pg "SELECT token FROM email_verifications ev JOIN users u ON u.id=ev.user_id WHERE u.email='$EMAIL'")
curl -sS "$BASE/api/auth/verify?token=$VTOKEN" >/dev/null
curl -sS -c "$CK" -X POST "$BASE/api/auth/login" \
  -H 'content-type: application/json' \
  -d "{\"email\":\"$EMAIL\",\"password\":\"hunter2hunter2\"}" >/dev/null

ORG_SLUG=$(curl -sS -b "$CK" "$BASE/api/orgs" \
  | python3 -c 'import sys,json; print(json.load(sys.stdin)[0]["slug"])')
[ -n "$ORG_SLUG" ] || { echo "FAIL no personal org bootstrapped"; exit 1; }
echo "  PASS personal org slug=$ORG_SLUG"

echo "[2/8] create project under personal org (sub-A)"
PROJ_RESP=$(curl -sS -b "$CK" -X POST "$BASE/admin/api/orgs/$ORG_SLUG/projects" \
  -H 'content-type: application/json' \
  -d '{"name":"grace-app"}')
PROJ_ID=$(echo "$PROJ_RESP" | python3 -c 'import sys,json; print(json.load(sys.stdin)["id"])')
[ -n "$PROJ_ID" ] || { echo "FAIL project create response: $PROJ_RESP"; exit 1; }
echo "  PASS project id=$PROJ_ID"

echo "[3/8] create public token labelled 'ios-prod' (sub-A)"
TOK_RESP=$(curl -sS -b "$CK" -X POST "$BASE/admin/api/projects/$PROJ_ID/tokens" \
  -H 'content-type: application/json' \
  -d '{"label":"ios-prod","kind":"public"}')
RAW=$(echo "$TOK_RESP" | python3 -c 'import sys,json; print(json.load(sys.stdin)["token"])')
TOK_ID=$(echo "$TOK_RESP" | python3 -c 'import sys,json; print(json.load(sys.stdin)["id"])')
[[ "$RAW" == st_pk_* && ${#RAW} == 32 ]] \
  || { echo "FAIL token format: $RAW (len=${#RAW})"; exit 1; }
echo "  PASS raw token format ok (len=${#RAW})"

echo "[4/8] list tokens — raw must NOT appear"
LIST=$(curl -sS -b "$CK" "$BASE/admin/api/projects/$PROJ_ID/tokens")
echo "$LIST" | grep -q "$RAW" \
  && { echo "FAIL: raw token leaked into list response"; exit 1; }
echo "  PASS list excludes raw value"
echo "$LIST" | grep -q "\"last4\":\"${RAW: -4}\"" \
  || { echo "FAIL: list missing last4=${RAW: -4}"; exit 1; }
echo "  PASS list shows last4=${RAW: -4}"

echo "[5/8] POST /v1/events with new token → 202 + issue created"
EVENT_ID=$(uuidgen | tr 'A-Z' 'a-z')
EVENT_BODY=$(cat <<JSON
{
  "id": "$EVENT_ID",
  "timestamp": "2026-05-09T12:34:56.789Z",
  "kind": "error",
  "platform": "javascript",
  "release": "grace-app@1.0.0+1",
  "environment": "prod",
  "device": { "os": "ios", "osVersion": "17.4" },
  "app": { "version": "1.0.0" },
  "error": {
    "type": "TypeError",
    "message": "hello from grace-app",
    "stack": [{ "file": "App.tsx", "line": 42, "inApp": true }]
  }
}
JSON
)
INGEST_STATUS=$(curl -sS -o /dev/null -w '%{http_code}' -X POST "$BASE/v1/events" \
  -H "Authorization: Bearer $RAW" \
  -H 'Sentori-Sdk: phase14-smoke/0.0.0' \
  -H 'content-type: application/json' \
  --data-raw "$EVENT_BODY")
assert_status "ingest with public token" 202 "$INGEST_STATUS"

# Allow the async write path to flush; in dev it's typically <100ms.
sleep 1

echo "[6/8] dashboard sees the issue"
ISSUES=$(curl -sS -b "$CK" "$BASE/admin/api/projects/$PROJ_ID/issues?limit=10")
COUNT=$(echo "$ISSUES" | python3 -c 'import sys,json; print(len(json.load(sys.stdin)))')
[ "$COUNT" -ge 1 ] || { echo "FAIL no issues in $ISSUES"; exit 1; }
TYPE=$(echo "$ISSUES" | python3 -c 'import sys,json; print(json.load(sys.stdin)[0]["errorType"])')
[ "$TYPE" = "TypeError" ] || { echo "FAIL errorType=$TYPE"; exit 1; }
echo "  PASS issue created (type=$TYPE, count=$COUNT)"

echo "[7/8] revoke token"
assert_status "revoke active token" 200 "$(curl -sS -b "$CK" -o /dev/null -w '%{http_code}' \
  -X DELETE "$BASE/admin/api/projects/$PROJ_ID/tokens/$TOK_ID")"

echo "[8/8] POST /v1/events with revoked token → 401"
assert_status "ingest with revoked token" 401 "$(curl -sS -o /dev/null -w '%{http_code}' \
  -X POST "$BASE/v1/events" \
  -H "Authorization: Bearer $RAW" \
  -H 'content-type: application/json' \
  --data-raw "$EVENT_BODY")"

echo
echo "Phase 14 SaaS onboarding smoke: ALL PASSED"
