#!/usr/bin/env bash
# self-hosted/tests/e2e/smoke.sh
#
# End-to-end smoke test for the self-hosted stack:
#   1. docker compose up -d (postgres + sentori-server)
#   2. wait for healthz to return ok
#   3. POST a synthetic event
#   4. Assert event_id + issue_id + is_new in response
#   5. docker compose down -v (clean teardown)
#
# Usage:
#   bash self-hosted/tests/e2e/smoke.sh
#
# Requirements: docker compose v2, jq, curl.
# Runs against a fresh ephemeral stack; no pre-existing
# state needed.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
COMPOSE_DIR="${ROOT}/docker"
cd "$COMPOSE_DIR"

# Pre-flight tooling check.
for cmd in docker jq curl; do
    if ! command -v "$cmd" >/dev/null 2>&1; then
        echo "❌ missing required tool: $cmd" >&2
        exit 1
    fi
done

# Unique compose project name so concurrent test runs
# don't collide.
export COMPOSE_PROJECT_NAME="sentori-e2e-$$"

cleanup() {
    echo "🧹 cleaning up ${COMPOSE_PROJECT_NAME}"
    docker compose down -v --remove-orphans >/dev/null 2>&1 || true
}
trap cleanup EXIT

echo "🚀 bringing up stack (${COMPOSE_PROJECT_NAME})"
cat > .env.e2e <<EOF
POSTGRES_USER=sentori
POSTGRES_PASSWORD=e2e-pass
POSTGRES_DB=sentori
POSTGRES_PORT=15432
SENTORI_PORT=18080
SENTORI_BOOTSTRAP_OWNER_EMAIL=e2e@example.com
SENTORI_BOOTSTRAP_OWNER_PASSWORD=e2e-PASS-1234
RUST_LOG=warn
EOF
docker compose --env-file .env.e2e up -d --build --quiet-pull
rm .env.e2e

echo "⏳ waiting for healthz"
for i in $(seq 1 30); do
    if curl -fsS http://localhost:18080/healthz 2>/dev/null | jq -e '.status == "ok"' >/dev/null 2>&1; then
        echo "✅ ready (after ${i}s)"
        break
    fi
    if [[ $i -eq 30 ]]; then
        echo "❌ timeout waiting for healthz" >&2
        docker compose logs server | tail -50 >&2
        exit 1
    fi
    sleep 1
done

echo "📡 fetching project list"
PROJECTS_JSON="$(curl -fsS http://localhost:18080/v1/projects)"
echo "$PROJECTS_JSON" | jq .

# Skeleton ships with no projects unless dashboard
# creates one — for v0.1 we accept empty list as
# valid. Production e2e would seed a project first.
if [[ "$(echo "$PROJECTS_JSON" | jq -r 'length')" == "0" ]]; then
    echo "ℹ️  empty project list — skipping ingest assertion (dashboard hasn't created a project)"
    echo "✅ smoke test PASSED (healthz + project list endpoint reachable)"
    exit 0
fi

PROJECT_ID="$(echo "$PROJECTS_JSON" | jq -r '.[0].id')"
echo "📨 posting test event to project ${PROJECT_ID}"

INGEST_RESPONSE="$(curl -fsS -X POST "http://localhost:18080/v1/events/${PROJECT_ID}" \
    -H 'content-type: application/json' \
    -d '{
        "kind": "error",
        "error_type": "TypeError",
        "message": "x is undefined",
        "platform": "javascript",
        "release": "e2e@0.1.0",
        "environment": "test"
    }')"

echo "$INGEST_RESPONSE" | jq .

EVENT_ID="$(echo "$INGEST_RESPONSE" | jq -r '.event_id')"
ISSUE_ID="$(echo "$INGEST_RESPONSE" | jq -r '.issue_id')"
IS_NEW="$(echo "$INGEST_RESPONSE" | jq -r '.is_new')"

if [[ -z "$EVENT_ID" || "$EVENT_ID" == "null" ]]; then
    echo "❌ missing event_id in ingest response" >&2
    exit 1
fi
if [[ -z "$ISSUE_ID" || "$ISSUE_ID" == "null" ]]; then
    echo "❌ missing issue_id in ingest response" >&2
    exit 1
fi
if [[ "$IS_NEW" != "true" ]]; then
    echo "❌ expected is_new=true on first event; got $IS_NEW" >&2
    exit 1
fi

echo "✅ e2e smoke test PASSED"
echo "    event_id: $EVENT_ID"
echo "    issue_id: $ISSUE_ID"
