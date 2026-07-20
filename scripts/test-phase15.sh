#!/usr/bin/env bash
#
# Phase 15 quota / usage smoke. Walks the full free-tier flow:
#   register → bootstrap personal org → tighten quota to 2 → ingest 4
#   events → verify 202/202/429/429 + Valkey usage + dropped + notified
#   flags + GET /api/orgs/{slug}/usage payload.
#
# Required env (sensible dev defaults provided):
#   SENTORI_BASE      base URL of the running server (default http://localhost:8080)
#   PG_CONTAINER      docker container name of postgres   (default sentori-pg)
#   VALKEY_CONTAINER  docker container name of valkey     (default sentori-vk)
#   PG_USER / PG_DB   pg login                            (default postgres / sentori)

set -euo pipefail

BASE="${SENTORI_BASE:-http://localhost:8080}"
PG_CONTAINER="${PG_CONTAINER:-sentori-pg}"
VK_CONTAINER="${VALKEY_CONTAINER:-sentori-vk}"
PG_USER="${PG_USER:-postgres}"
PG_DB="${PG_DB:-sentori}"

pg() { docker exec "$PG_CONTAINER" psql -U "$PG_USER" -d "$PG_DB" -tA -c "$1" | tr -d '\r' | head -1; }
vk() { docker exec "$VK_CONTAINER" valkey-cli "$@"; }

RUN_ID=$(date +%s%N)
EMAIL="luna-${RUN_ID}@test.local"
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

assert_eq() {
  local label=$1 want=$2 got=$3
  if [ "$got" != "$want" ]; then
    echo "FAIL $label — expected $want, got $got" >&2
    exit 1
  fi
  echo "  PASS $label = $got"
}

echo "Phase 15 quota smoke (run $RUN_ID)"
echo

echo "[1/7] register + verify + login + bootstrap personal org"
curl -sS -X POST "$BASE/api/auth/register" \
  -H 'content-type: application/json' \
  -d "{\"email\":\"$EMAIL\",\"password\":\"hunter2hunter2\"}" >/dev/null
VTOK=$(pg "SELECT token FROM email_verifications ev JOIN users u ON u.id=ev.user_id WHERE u.email='$EMAIL'")
curl -sS "$BASE/api/auth/verify?token=$VTOK" >/dev/null
curl -sS -c "$CK" -X POST "$BASE/api/auth/login" \
  -H 'content-type: application/json' \
  -d "{\"email\":\"$EMAIL\",\"password\":\"hunter2hunter2\"}" >/dev/null

ORG_SLUG=$(curl -sS -b "$CK" "$BASE/api/orgs" \
  | python3 -c 'import sys,json; print(json.load(sys.stdin)[0]["slug"])')
ORG_ID=$(pg "SELECT id FROM orgs WHERE slug='$ORG_SLUG'")
echo "  PASS org=$ORG_SLUG id=$ORG_ID"

echo "[2/7] tighten quota row to event_limit_monthly=2 (free tier default would be 100k)"
pg "UPDATE org_quotas SET event_limit_monthly=2 WHERE org_id='$ORG_ID'" >/dev/null
LIMIT=$(pg "SELECT event_limit_monthly FROM org_quotas WHERE org_id='$ORG_ID'")
assert_eq "configured limit" "2" "$LIMIT"

echo "[3/7] create project + public token"
PROJ_ID=$(curl -sS -b "$CK" -X POST "$BASE/admin/api/orgs/$ORG_SLUG/projects" \
  -H 'content-type: application/json' -d '{"name":"luna-app"}' \
  | python3 -c 'import sys,json; print(json.load(sys.stdin)["id"])')
RAW=$(curl -sS -b "$CK" -X POST "$BASE/admin/api/projects/$PROJ_ID/tokens" \
  -H 'content-type: application/json' -d '{"label":"smoke","kind":"public"}' \
  | python3 -c 'import sys,json; print(json.load(sys.stdin)["token"])')
[[ "$RAW" == st_pk_* ]] || { echo "FAIL token format"; exit 1; }
echo "  PASS token minted"

post_one() {
  local body='{"id":"'$(uuidgen | tr 'A-Z' 'a-z')'","timestamp":"2026-05-09T12:34:56.789Z","kind":"error","platform":"javascript","release":"luna@1.0.0+1","environment":"prod","device":{"os":"ios","osVersion":"17"},"app":{"version":"1.0.0"},"error":{"type":"L","message":"x","stack":[{"file":"a.ts","line":1,"inApp":true}]}}'
  curl -sS -o /dev/null -w '%{http_code}' -X POST "$BASE/v1/events" \
    -H "Authorization: Bearer $RAW" -H 'content-type: application/json' --data-raw "$body"
}

echo "[4/7] ingest 4 events — first two admit, latter two 429"
assert_status "event 1 (50%)" 202 "$(post_one)"
assert_status "event 2 (100%, crosses 80 + 100)" 202 "$(post_one)"
assert_status "event 3 (over)" 429 "$(post_one)"
assert_status "event 4 (over)" 429 "$(post_one)"

echo "[5/7] Valkey counters + notified flags"
PERIOD=$(date -u +%Y%m)
USAGE=$(vk GET "usage:$ORG_ID:$PERIOD")
DROPPED=$(vk GET "dropped:$ORG_ID:$PERIOD")
NOTIFIED_80=$(vk EXISTS "notified:80:$ORG_ID:$PERIOD")
NOTIFIED_100=$(vk EXISTS "notified:100:$ORG_ID:$PERIOD")
assert_eq "valkey usage"        "2" "$USAGE"
assert_eq "valkey dropped"      "2" "$DROPPED"
assert_eq "notified:80 flag"    "1" "$NOTIFIED_80"
assert_eq "notified:100 flag"   "1" "$NOTIFIED_100"

echo "[6/7] 429 body carries resetAt RFC3339"
RESET=$(curl -sS -X POST "$BASE/v1/events" \
  -H "Authorization: Bearer $RAW" -H 'content-type: application/json' \
  -d '{"id":"'$(uuidgen | tr 'A-Z' 'a-z')'","timestamp":"2026-05-09T12:34:56.789Z","kind":"error","platform":"javascript","release":"luna@1.0.0+1","environment":"prod","device":{"os":"ios","osVersion":"17"},"app":{"version":"1.0.0"},"error":{"type":"L","message":"x","stack":[{"file":"a.ts","line":1,"inApp":true}]}}' \
  | python3 -c 'import sys,json; print(json.load(sys.stdin)["resetAt"])')
[[ "$RESET" =~ ^[0-9]{4}-[0-9]{2}-01T00:00:00Z$ ]] \
  || { echo "FAIL resetAt format: $RESET"; exit 1; }
echo "  PASS resetAt=$RESET"

echo "[7/7] GET /api/orgs/{slug}/usage reflects live counters"
USAGE_JSON=$(curl -sS -b "$CK" "$BASE/api/orgs/$ORG_SLUG/usage")
PLAN=$(echo "$USAGE_JSON" | python3 -c 'import sys,json; print(json.load(sys.stdin)["plan"])')
EVENT_COUNT=$(echo "$USAGE_JSON" | python3 -c 'import sys,json; print(json.load(sys.stdin)["eventCount"])')
DROPPED_COUNT=$(echo "$USAGE_JSON" | python3 -c 'import sys,json; print(json.load(sys.stdin)["droppedCount"])')
PCT=$(echo "$USAGE_JSON" | python3 -c 'import sys,json; print(int(json.load(sys.stdin)["percentUsed"]))')
assert_eq "usage.plan"        "free" "$PLAN"
assert_eq "usage.eventCount"  "2"    "$EVENT_COUNT"
assert_eq "usage.percentUsed" "100"  "$PCT"
[ "$DROPPED_COUNT" -ge 2 ] \
  || { echo "FAIL droppedCount=$DROPPED_COUNT (want >=2)"; exit 1; }
echo "  PASS usage.droppedCount=$DROPPED_COUNT (>=2)"

echo
echo "Phase 15 quota smoke: ALL PASSED"
