#!/usr/bin/env bash
# An issue arrives, and the team's webhook is actually POSTed.
#
#   bash scripts/webhook-alert-e2e.sh
#
# Issue alerts were email-only, and `spawn_issue_notification` returned
# early when no SMTP was configured — so a self-hosted instance with no
# mail server got no alerts at all and nothing said so. The operator
# finds out by never hearing about an outage.
#
# `WebhookTransport` had been in the notifier crate the whole time,
# with tests and a doc comment teaching people to use it, and the
# server never registered it. This checks that it does now, from
# outside the process: a receiver on its own port, and the assertion is
# that something knocked on it. A mock inside the server could not
# answer the question, because the question *is* whether the transport
# is reached.
#
# Run without SMTP on purpose. That is the configuration this channel
# exists for, and the one where an email-shaped test would pass while
# the product sent nothing.

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

PORT="$(free_port 8402)"
PGPORT="$(free_port 55457)"
HOOKPORT="$(free_port 8403)"
PG_CONTAINER="sentori-webhook-pg-$$"
BASE="http://127.0.0.1:${PORT}"
HOOK_URL="http://127.0.0.1:${HOOKPORT}/team-room"
LOG="$(mktemp)"
HOOK_FILE="$(mktemp)"
SERVER_PID=""
SINK_PID=""
STARTED_BREW_PG=""

cleanup() {
    [ -n "$SERVER_PID" ] && kill "$SERVER_PID" 2>/dev/null || true
    [ -n "$SINK_PID" ] && kill "$SINK_PID" 2>/dev/null || true
    docker rm -f "$PG_CONTAINER" >/dev/null 2>&1 || true
    [ -n "$STARTED_BREW_PG" ] && brew services stop "$STARTED_BREW_PG" >/dev/null 2>&1 || true
}
trap cleanup EXIT

echo "→ postgres"
DBNAME=sentori_webhook
# shellcheck source=scripts/lib/local-postgres.sh
. "${ROOT}/scripts/lib/local-postgres.sh"
start_local_postgres

echo "→ a receiver on its own port"
: > "$HOOK_FILE"
python3 "$ROOT/scripts/lib/webhook-sink.py" "$HOOKPORT" "$HOOK_FILE" &
SINK_PID=$!
for _ in $(seq 1 30); do
    curl -fsS -X POST "http://127.0.0.1:${HOOKPORT}/ping" -d '{}' >/dev/null 2>&1 && break
    sleep 0.2
done
: > "$HOOK_FILE"

echo "→ server, deliberately with no SMTP"
(cd self-hosted/server && cargo build --quiet)
SENTORI_BIND="127.0.0.1:${PORT}" SENTORI_DATABASE_URL="$DB" \
    SENTORI_OWNER_EMAIL=hook@example.com SENTORI_OWNER_PASSWORD=hook-password-long-enough \
    SENTORI_BASE_URL="$BASE" \
    self-hosted/server/target/debug/sentori-server > "$LOG" 2>&1 &
SERVER_PID=$!
for _ in $(seq 1 60); do curl -fsS "${BASE}/healthz" >/dev/null 2>&1 && break; sleep 1; done
curl -fsS "${BASE}/healthz" >/dev/null || { echo "server never came up:"; tail -20 "$LOG"; exit 1; }

# The configuration this channel exists for. If a future change makes
# SMTP required, this says so here rather than in a customer's logs.
[[ "$(curl -fsS -b /dev/null "${BASE}/healthz" >/dev/null 2>&1; grep -c 'SENTORI_SMTP_HOST unset' "$LOG")" -ge 1 ]] \
    || { echo "expected the server to report SMTP unset — this test is meant to run without it" >&2; exit 1; }

echo "→ project, with the webhook set"
JAR="$(mktemp)"
curl -fsS -c "$JAR" -X POST "${BASE}/auth/login" -H 'content-type: application/json' \
    -d '{"email":"hook@example.com","password":"hook-password-long-enough"}' >/dev/null
PROJECT="$(curl -fsS -b "$JAR" -X POST "${BASE}/admin/api/projects" \
    -H 'content-type: application/json' -d '{"name":"webhook-e2e","platform":"ios"}' \
    | python3 -c 'import sys,json;print(json.load(sys.stdin)["id"])')"
curl -fsS -b "$JAR" -X PATCH "${BASE}/admin/api/projects/${PROJECT}" \
    -H 'content-type: application/json' -d "{\"webhookUrl\":\"${HOOK_URL}\"}" >/dev/null

# Read it back, because a PATCH that answers ok and stores nothing is
# the shape this whole script exists to disprove.
STORED="$(curl -fsS -b "$JAR" "${BASE}/admin/api/projects/${PROJECT}" \
    | python3 -c 'import sys,json;print(json.load(sys.stdin).get("webhookUrl") or "")')"
[[ "$STORED" == "$HOOK_URL" ]] \
    || { echo "✗ the webhook URL was accepted and read back as '${STORED}'" >&2; exit 1; }

TOKEN="$(curl -fsS -b "$JAR" -X POST "${BASE}/admin/api/projects/${PROJECT}/tokens" \
    -H 'content-type: application/json' -d '{"name":"hook","scope":"ingest"}' \
    | python3 -c 'import sys,json;print(json.load(sys.stdin)["token"])')"

echo "→ an error, which is a new issue"
curl -fsS -X POST "${BASE}/v1/events" -H "Authorization: Bearer ${TOKEN}" \
    -H 'content-type: application/json' \
    -d '{"kind":"error","occurredAt":"2026-09-30T00:00:00Z","platform":"ios",
 "release":"hook@1.0.0+1","environment":"test",
 "payload":{"error":{"type":"PaymentDeclined","message":"card refused","stack":[]}}}' >/dev/null

echo "→ did anything knock on the receiver"
for _ in $(seq 1 40); do
    [ -s "$HOOK_FILE" ] && break
    sleep 0.5
done
python3 "$ROOT/scripts/lib/check-webhook-alert.py" "$HOOK_FILE" "$LOG"
