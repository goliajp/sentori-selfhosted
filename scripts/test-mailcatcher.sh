#!/usr/bin/env bash
#
# Phase 16 sub-E: notifier email integration smoke against mailpit.
#
# Spins up an axllent/mailpit container, points the running Sentori
# server at it (SENTORI_SMTP_HOST=127.0.0.1 SMTP_PORT=1025 SMTP_TLS=plain),
# triggers a register → email-verification flow, asserts a message
# lands in mailpit's inbox with the right subject + verify-link body.
#
# Linux CI is the canonical environment. macOS Docker Desktop has
# known port-forwarding quirks that can swallow the SMTP banner — if
# you see `incomplete response` on darwin, run this in Docker-in-Docker
# or wait for CI to validate.
#
# Required: server running with SENTORI_SMTP_HOST=127.0.0.1
#           SENTORI_SMTP_PORT=1025 SENTORI_SMTP_TLS=plain configured.

set -euo pipefail

PG_CONTAINER="${PG_CONTAINER:-sentori-pg}"
PG_USER="${PG_USER:-postgres}"
PG_DB="${PG_DB:-sentori}"
BASE="${SENTORI_BASE:-http://localhost:8080}"
MAIL_HTTP="${MAIL_HTTP:-http://localhost:1080}"

pg() { docker exec "$PG_CONTAINER" psql -U "$PG_USER" -d "$PG_DB" -tA -c "$1" | tr -d '\r' | head -1; }

start_mailpit() {
  if docker ps --format '{{.Names}}' | grep -q '^sentori-mc$'; then
    return
  fi
  docker run -d --name sentori-mc \
    -p 1025:1025 -p 1080:8025 \
    axllent/mailpit:latest >/dev/null
  # Mailpit is up in <1s on Linux; on macOS Docker the SMTP socket
  # accepts but doesn't always forward the banner — see header.
  for _ in $(seq 1 20); do
    if curl -sf "$MAIL_HTTP/api/v1/messages" >/dev/null 2>&1; then
      return
    fi
    sleep 0.5
  done
  echo "FAIL mailpit didn't come up at $MAIL_HTTP" >&2
  exit 1
}

reset_mailpit() {
  curl -sS -X DELETE "$MAIL_HTTP/api/v1/messages" >/dev/null
}

start_mailpit
reset_mailpit

EMAIL="mailtest-$(date +%s%N)@test.local"
echo "[1/3] register $EMAIL"
curl -sS -X POST "$BASE/api/auth/register" \
  -H 'content-type: application/json' \
  -d "{\"email\":\"$EMAIL\",\"password\":\"hunter2hunter2\"}" >/dev/null

echo "[2/3] poll mailpit for the verification email"
for _ in $(seq 1 30); do
  TOTAL=$(curl -sS "$MAIL_HTTP/api/v1/messages" \
    | python3 -c 'import sys,json; print(json.load(sys.stdin).get("total", 0))')
  if [ "$TOTAL" -ge 1 ]; then
    break
  fi
  sleep 1
done
[ "${TOTAL:-0}" -ge 1 ] || { echo "FAIL no email arrived (mailpit total=$TOTAL)" >&2; exit 1; }
echo "  PASS mailpit total=$TOTAL"

LATEST=$(curl -sS "$MAIL_HTTP/api/v1/messages" \
  | python3 -c 'import sys,json; print(json.load(sys.stdin)["messages"][0]["ID"])')
SUBJECT=$(curl -sS "$MAIL_HTTP/api/v1/message/$LATEST" \
  | python3 -c 'import sys,json; print(json.load(sys.stdin)["Subject"])')
TEXT=$(curl -sS "$MAIL_HTTP/api/v1/message/$LATEST" \
  | python3 -c 'import sys,json; print(json.load(sys.stdin).get("Text", ""))')

echo "[3/3] assert subject + body"
[[ "$SUBJECT" == *"Verify your email"* ]] \
  || { echo "FAIL subject=$SUBJECT" >&2; exit 1; }
echo "  PASS subject=$SUBJECT"
[[ "$TEXT" == *"$BASE/verify?token="* ]] \
  || { echo "FAIL body missing verify link; got: $TEXT" >&2; exit 1; }
echo "  PASS body contains verify link"

# Verify the link actually flips email_verified.
TOKEN=$(echo "$TEXT" | grep -oE '\?token=[a-f0-9]+' | head -1 | sed 's/^?token=//')
curl -sS "$BASE/api/auth/verify?token=$TOKEN" >/dev/null
VERIFIED=$(pg "SELECT email_verified FROM users WHERE email='$EMAIL'")
[ "$VERIFIED" = "t" ] || { echo "FAIL verify endpoint didn't flip email_verified"; exit 1; }
echo "  PASS verify flipped email_verified=true"

echo
echo "mailcatcher integration smoke: ALL PASSED"
