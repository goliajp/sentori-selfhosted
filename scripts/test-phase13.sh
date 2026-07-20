#!/usr/bin/env bash
#
# Phase 13 multi-tenant integration smoke. Exits non-zero on the first
# failed assertion so it's safe to wire into CI after `cargo run` is up.
#
# Required env (or sensible dev defaults are used):
#   SENTORI_BASE      base URL of the running server (default http://localhost:8080)
#   SENTORI_DEV_TOKEN dev Bearer token the server was started with (default devtoken)
#   PG_CONTAINER      docker container name of postgres   (default sentori-pg)
#   PG_USER / PG_DB   pg login                            (default postgres / sentori)
#
# Reads token / invite values directly from the DB because there's no
# email captured locally (SMTP is not configured in dev).

set -euo pipefail

BASE="${SENTORI_BASE:-http://localhost:8080}"
DEV_TOKEN="${SENTORI_DEV_TOKEN:-devtoken}"
PG_CONTAINER="${PG_CONTAINER:-sentori-pg}"
PG_USER="${PG_USER:-postgres}"
PG_DB="${PG_DB:-sentori}"

pg() { docker exec "$PG_CONTAINER" psql -U "$PG_USER" -d "$PG_DB" -tA -c "$1" | tr -d '\r'; }

# Suffix every fixture with a fresh slug so the script can be re-run
# against the same DB without unique-constraint conflicts.
RUN_ID=$(date +%s%N)
ALICE="alice-${RUN_ID}@test.local"
BOB="bob-${RUN_ID}@test.local"
ORG_A="acme-${RUN_ID}"
ORG_B="globex-${RUN_ID}"

ALICE_CK=$(mktemp)
BOB_CK=$(mktemp)
trap 'rm -f "$ALICE_CK" "$BOB_CK"' EXIT

assert_status() {
  local label=$1 want=$2 got=$3
  if [ "$got" != "$want" ]; then
    echo "FAIL $label — expected http=$want, got $got" >&2
    exit 1
  fi
  echo "  PASS $label (http=$got)"
}

reg_login() {
  local email=$1 cookie=$2
  curl -sS -X POST "$BASE/api/auth/register" \
    -H 'content-type: application/json' \
    -d "{\"email\":\"$email\",\"password\":\"hunter2hunter2\"}" >/dev/null
  local token
  token=$(pg "SELECT token FROM email_verifications ev JOIN users u ON u.id=ev.user_id WHERE u.email='$email'")
  curl -sS "$BASE/api/auth/verify?token=$token" >/dev/null
  curl -sS -c "$cookie" -X POST "$BASE/api/auth/login" \
    -H 'content-type: application/json' \
    -d "{\"email\":\"$email\",\"password\":\"hunter2hunter2\"}" >/dev/null
}

create_org() {
  local cookie=$1 slug=$2
  curl -sS -b "$cookie" -X POST "$BASE/api/orgs" \
    -H 'content-type: application/json' \
    -d "{\"slug\":\"$slug\",\"name\":\"$slug\"}" >/dev/null
  pg "SELECT id FROM orgs WHERE slug='$slug'"
}

seed_project() {
  local org_id=$1 name=$2
  # psql emits "INSERT 0 1" after the RETURNING row; keep just the uuid.
  pg "INSERT INTO projects (id, name, org_id) VALUES (uuidv7(), '$name', '$org_id') RETURNING id" \
    | head -1
}

http_status() {
  local cookie=$1 path=$2
  curl -sS -b "$cookie" -o /dev/null -w '%{http_code}' "$BASE$path"
}

echo "Phase 13 multi-tenant smoke (run $RUN_ID)"
echo

echo "[1/6] register + verify + login alice & bob"
reg_login "$ALICE" "$ALICE_CK"
reg_login "$BOB" "$BOB_CK"

echo "[2/6] confirm sub-H bootstrap created personal orgs"
ALICE_PERSONAL=$(pg "SELECT count(*) FROM orgs o JOIN memberships m ON m.org_id=o.id JOIN users u ON u.id=m.user_id WHERE u.email='$ALICE'")
[ "$ALICE_PERSONAL" = "1" ] || { echo "FAIL alice should have 1 auto org, has $ALICE_PERSONAL"; exit 1; }
echo "  PASS alice has 1 auto-bootstrapped org"

echo "[3/6] create explicit orgs A & B and seed a project under each"
ORG_A_ID=$(create_org "$ALICE_CK" "$ORG_A")
ORG_B_ID=$(create_org "$BOB_CK"   "$ORG_B")
PROJECT_A=$(seed_project "$ORG_A_ID" "alice-app")
PROJECT_B=$(seed_project "$ORG_B_ID" "bob-app")

echo "[4/6] cross-org isolation: each user sees only their projects"
ALICE_PROJECTS=$(curl -sS -b "$ALICE_CK" "$BASE/admin/api/projects" | python3 -c 'import sys,json; ds=json.load(sys.stdin); print(",".join(p["name"] for p in ds))')
BOB_PROJECTS=$(curl -sS -b "$BOB_CK"   "$BASE/admin/api/projects" | python3 -c 'import sys,json; ds=json.load(sys.stdin); print(",".join(p["name"] for p in ds))')
[[ "$ALICE_PROJECTS" == *"alice-app"* && "$ALICE_PROJECTS" != *"bob-app"* ]] \
  || { echo "FAIL alice's projects=$ALICE_PROJECTS"; exit 1; }
[[ "$BOB_PROJECTS"   == *"bob-app"*   && "$BOB_PROJECTS"   != *"alice-app"* ]] \
  || { echo "FAIL bob's projects=$BOB_PROJECTS"; exit 1; }
echo "  PASS alice sees only her projects: $ALICE_PROJECTS"
echo "  PASS bob   sees only his projects: $BOB_PROJECTS"

assert_status "alice GET bob's project issues -> 403" \
  403 "$(http_status "$ALICE_CK" "/admin/api/projects/$PROJECT_B/issues")"
assert_status "bob   GET alice's project issues -> 403" \
  403 "$(http_status "$BOB_CK"   "/admin/api/projects/$PROJECT_A/issues")"
assert_status "alice GET her own project issues -> 200" \
  200 "$(http_status "$ALICE_CK" "/admin/api/projects/$PROJECT_A/issues")"
assert_status "dev token GET bob's project issues -> 200 (super)" \
  200 "$(curl -sS -H "Authorization: Bearer $DEV_TOKEN" -o /dev/null -w '%{http_code}' "$BASE/admin/api/projects/$PROJECT_B/issues")"

echo "[5/6] invite bob to org A, bob accepts, isolation lifts"
curl -sS -b "$ALICE_CK" -X POST "$BASE/api/orgs/$ORG_A/invites" \
  -H 'content-type: application/json' \
  -d "{\"email\":\"$BOB\",\"role\":\"member\"}" >/dev/null
INV=$(pg "SELECT token FROM org_invites WHERE email='$BOB' AND used_at IS NULL")
ACCEPT_RESP=$(curl -sS -b "$BOB_CK" -X POST "$BASE/api/invites/$INV/accept")
echo "$ACCEPT_RESP" | grep -q "\"orgSlug\":\"$ORG_A\"" \
  || { echo "FAIL accept response: $ACCEPT_RESP"; exit 1; }
echo "  PASS bob accepted invite to $ORG_A"

assert_status "bob GET alice's project issues after join -> 200" \
  200 "$(http_status "$BOB_CK" "/admin/api/projects/$PROJECT_A/issues")"

echo "[6/6] mismatched-email invite is rejected"
curl -sS -b "$ALICE_CK" -X POST "$BASE/api/orgs/$ORG_A/invites" \
  -H 'content-type: application/json' \
  -d '{"email":"someone-else@test.local","role":"member"}' >/dev/null
WRONG_INV=$(pg "SELECT token FROM org_invites WHERE email='someone-else@test.local' AND used_at IS NULL ORDER BY created_at DESC LIMIT 1")
assert_status "bob accepting an invite for someone-else@... -> 403" \
  403 "$(curl -sS -b "$BOB_CK" -o /dev/null -w '%{http_code}' -X POST "$BASE/api/invites/$WRONG_INV/accept")"

echo
echo "Phase 13 multi-tenant smoke: ALL PASSED"
