#!/usr/bin/env bash
# self-hosted/tests/e2e/smoke.sh
#
# The shipped stack, from `docker compose up` to an event you can
# read back — against the real image, the real compose file and the
# real migrations.
#
#   bash self-hosted/tests/e2e/smoke.sh
#
# Requires docker compose v2, jq, curl.
#
# It asserts the things that have actually broken:
#
#   - the owner bootstrap prints a password you can log in with
#     (a generated password that never reaches the log is a locked
#     instance);
#   - an ingest token can post an event and gets a real issue back;
#   - **a resent event id is accepted rather than 500** — the case a
#     mobile client hits whenever a response is lost, which until
#     server 2.18.0 was a primary-key violation dressed as a server
#     fault, and which our own contract told the SDK to retry;
#   - a resend does not double-count the issue;
#   - a batch reports one outcome per event.
#
# There is no skip path. The previous version of this file exited 0
# with "empty project list — skipping ingest assertion", so the only
# thing it could ever prove was that healthz answered.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "${ROOT}/docker"

for cmd in docker jq curl; do
    command -v "$cmd" >/dev/null 2>&1 || { echo "missing required tool: $cmd" >&2; exit 1; }
done

PORT="${E2E_PORT:-18080}"
BASE="http://127.0.0.1:${PORT}"
export COMPOSE_PROJECT_NAME="sentori-e2e-$$"
ENV_FILE=".env.e2e.$$"
JAR="$(mktemp)"

cleanup() {
    # SENTORI_E2E_KEEP leaves the stack up so a passing run can be
    # inspected — logs read, the database dumped, a request replayed
    # by hand. It prints how to reach it and how to remove it, because
    # a stack left running that nobody knows the name of is litter.
    if [ -n "${SENTORI_E2E_KEEP:-}" ]; then
        echo
        echo "→ stack kept: ${COMPOSE_PROJECT_NAME}  (${BASE})"
        echo "  remove it with:"
        echo "    COMPOSE_PROJECT_NAME=${COMPOSE_PROJECT_NAME} \\"
        echo "      docker compose --env-file ${ENV_FILE} down -v --remove-orphans"
        echo "    rm -f ${ENV_FILE}"
        rm -f "$JAR"
        return
    fi
    docker compose --env-file "$ENV_FILE" down -v --remove-orphans >/dev/null 2>&1 || true
    rm -f "$ENV_FILE" "$JAR"
}
trap cleanup EXIT

cat > "$ENV_FILE" <<EOF
POSTGRES_PASSWORD=e2e-pass
SENTORI_OWNER_EMAIL=e2e@example.com
SENTORI_BASE_URL=${BASE}
SENTORI_PORT=${PORT}
RUST_LOG=warn
SENTORI_PG_IMAGE=${SENTORI_PG_IMAGE:-postgres:18-alpine}
EOF

echo "→ up (${COMPOSE_PROJECT_NAME})"
docker compose --env-file "$ENV_FILE" up -d --build --quiet-pull

echo "→ waiting for healthz"
for i in $(seq 1 60); do
    if curl -fsS "${BASE}/healthz" 2>/dev/null | jq -e '.status == "ok"' >/dev/null 2>&1; then
        echo "  ready after ${i}s"
        break
    fi
    if [[ $i -eq 60 ]]; then
        echo "timeout waiting for healthz" >&2
        docker compose --env-file "$ENV_FILE" logs sentori | tail -50 >&2
        exit 1
    fi
    sleep 1
done

# The generated owner password is printed once, at boot. An operator
# who cannot find it has an instance nobody can log into.
# `|| true` because a `grep` that matches nothing is a non-zero exit,
# and under `set -e` that kills the script before the check below can
# say what went wrong — which is how this step failed silently once.
PASSWORD=""
for _ in $(seq 1 10); do
    PASSWORD="$(docker compose --env-file "$ENV_FILE" logs sentori 2>/dev/null \
        | tr -d '\r' | sed $'s/\033\[[0-9;]*m//g' \
        | grep -o 'password=[^ ]*' | head -1 | cut -d= -f2 || true)"
    [[ -n "$PASSWORD" ]] && break
    sleep 1
done
if [[ -z "$PASSWORD" ]]; then
    echo "no generated password in the boot log — an instance nobody can log into" >&2
    docker compose --env-file "$ENV_FILE" logs sentori | tail -30 >&2
    exit 1
fi

echo "→ sign in"
curl -fsS -c "$JAR" -X POST "${BASE}/auth/login" -H 'content-type: application/json' \
    -d "{\"email\":\"e2e@example.com\",\"password\":\"${PASSWORD}\"}" >/dev/null

echo "→ project + ingest token"
PROJECT_ID="$(curl -fsS -b "$JAR" -X POST "${BASE}/admin/api/projects" \
    -H 'content-type: application/json' \
    -d '{"name":"e2e","platform":"react-native"}' | jq -r '.id')"
[[ -n "$PROJECT_ID" && "$PROJECT_ID" != "null" ]] || { echo "no project id" >&2; exit 1; }

TOKEN="$(curl -fsS -b "$JAR" -X POST "${BASE}/admin/api/projects/${PROJECT_ID}/tokens" \
    -H 'content-type: application/json' \
    -d '{"name":"e2e-app","scope":"ingest"}' | jq -r '.token')"
[[ "$TOKEN" == st_* ]] || { echo "minted token does not look like st_… : $TOKEN" >&2; exit 1; }

EVENT_ID="019fe900-0000-7000-8000-0000000e2e01"
event_body() {
    cat <<EOF
{"id":"${EVENT_ID}","kind":"error","occurredAt":"2026-08-10T06:00:00Z",
 "platform":"javascript","release":"e2e@1.0.0+1","environment":"test",
 "payload":{"error":{"type":"TypeError","message":"x is undefined","stack":[]}}}
EOF
}

echo "→ ingest"
FIRST="$(curl -fsS -X POST "${BASE}/v1/events" -H "Authorization: Bearer ${TOKEN}" \
    -H 'content-type: application/json' -d "$(event_body)")"
ISSUE_ID="$(echo "$FIRST" | jq -r '.issueId')"
[[ "$(echo "$FIRST" | jq -r '.eventId')" == "$EVENT_ID" ]] \
    || { echo "server did not keep the client-minted id: $FIRST" >&2; exit 1; }
[[ -n "$ISSUE_ID" && "$ISSUE_ID" != "null" ]] || { echo "no issueId: $FIRST" >&2; exit 1; }
[[ "$(echo "$FIRST" | jq -r '.isNewIssue')" == "true" ]] \
    || { echo "expected isNewIssue=true on the first event: $FIRST" >&2; exit 1; }

echo "→ resend the same id (lost-response case)"
STATUS="$(curl -s -o /tmp/e2e-resend.$$ -w '%{http_code}' -X POST "${BASE}/v1/events" \
    -H "Authorization: Bearer ${TOKEN}" -H 'content-type: application/json' \
    -d "$(event_body)")"
SECOND="$(cat /tmp/e2e-resend.$$)"; rm -f /tmp/e2e-resend.$$
[[ "$STATUS" == "202" ]] \
    || { echo "resend returned ${STATUS}, want 202 — an SDK retries a 5xx forever: $SECOND" >&2; exit 1; }
[[ "$(echo "$SECOND" | jq -r '.isNewIssue')" == "false" ]] \
    || { echo "resend reported a new issue: $SECOND" >&2; exit 1; }
[[ "$(echo "$SECOND" | jq -r '.issueId')" == "$ISSUE_ID" ]] \
    || { echo "resend landed on a different issue: $SECOND" >&2; exit 1; }

echo "→ the resend did not count twice"
COUNT="$(curl -fsS -b "$JAR" "${BASE}/admin/api/issues?projectId=${PROJECT_ID}" \
    | jq -r --arg id "$ISSUE_ID" '.issues[] | select(.id == $id) | .eventCount')"
[[ "$COUNT" == "1" ]] || { echo "issue eventCount is ${COUNT}, want 1" >&2; exit 1; }

echo "→ batch"
BATCH="$(curl -fsS -X POST "${BASE}/v1/events:batch" -H "Authorization: Bearer ${TOKEN}" \
    -H 'content-type: application/json' -d '{"events":[
      {"kind":"warn","occurredAt":"2026-08-10T06:01:00Z","platform":"ios",
       "release":"e2e@1.0.0+1","environment":"test","name":"dead_button",
       "surface":{"screen":"Checkout","element":"PayButton"},"payload":{}},
      {"kind":"trace","occurredAt":"2026-08-10T06:01:01Z","platform":"ios",
       "release":"e2e@1.0.0+1","environment":"test","name":"app.launch","payload":{}}]}')"
[[ "$(echo "$BATCH" | jq -r '.accepted')" == "2" ]] \
    || { echo "batch accepted != 2: $BATCH" >&2; exit 1; }
[[ "$(echo "$BATCH" | jq -r '.outcomes | length')" == "2" ]] \
    || { echo "batch did not report one outcome per event: $BATCH" >&2; exit 1; }

echo "→ assertStats piggybacked on the batch reach the instruments panel"
# The batch step sent none of these until 2026-08-19, so nothing on
# this path had ever been exercised end to end — and the call site
# logs a failure as a warning rather than failing the batch, so a
# broken store here would have shown up as an empty panel and nothing
# else. Sending twice also pins the accumulate-and-stamp half of the
# upsert, which is where an overwrite would hide.
for pass_fail in "4 0" "3 1"; do
    set -- $pass_fail
    curl -fsS -X POST "${BASE}/v1/events:batch" -H "Authorization: Bearer ${TOKEN}" \
        -H 'content-type: application/json' \
        -d "{\"events\":[],\"assertStats\":[{\"name\":\"pay.token-fresh\",
             \"release\":\"e2e@1.0.0+1\",\"passDelta\":$1,\"failDelta\":$2}]}" >/dev/null
done
INSTR="$(curl -fsS -b "$JAR" "${BASE}/admin/api/projects/${PROJECT_ID}/instruments")"
echo "$INSTR" | jq -e '[.asserts[] | select(.name == "pay.token-fresh")] | length == 1' > /dev/null \
    || { echo "the assert never reached the panel: $INSTR" >&2; exit 1; }
# 4+3 and 0+1: the upsert accumulates rather than overwriting.
echo "$INSTR" | jq -e '.asserts[] | select(.name == "pay.token-fresh")
                       | .passCount == 7 and .failCount == 1' > /dev/null \
    || { echo "assert counts did not accumulate: $INSTR" >&2; exit 1; }
# And the conditional stamps fired — both, because both deltas were
# nonzero across the two batches.
echo "$INSTR" | jq -e '.asserts[] | select(.name == "pay.token-fresh")
                       | (.lastPassAt != null) and (.lastFailAt != null)' > /dev/null \
    || { echo "the CASE WHEN stamps did not fire: $INSTR" >&2; exit 1; }

# ── resolve → regression, the product's central mechanic ─────────
#
# "Fixed in release X" means only a recurrence in X or newer reopens
# the case. An older release still crashing is the build you already
# fixed, not a regression — and a fix that reopens on it teaches
# people to ignore the signal. There is a lot of machinery behind
# that sentence (release ordering, the anchor, the weak time
# fallback) and no test outside this file.
echo "→ two releases, oldest first"
for r in "reg@1.0.0+1" "reg@1.0.0+2"; do
    curl -fsS -X POST "${BASE}/v1/deploys" -H "Authorization: Bearer ${TOKEN}" \
        -H 'content-type: application/json' -d "{\"release\":\"${r}\"}" >/dev/null
    sleep 1   # created_at ordering is what the anchor compares
done

reg_event() {  # $1 = release
    cat <<EOF
{"kind":"error","occurredAt":"2026-08-10T06:02:00Z","platform":"ios",
 "release":"$1","environment":"test",
 "payload":{"error":{"type":"RangeError","message":"regression probe","stack":[]}}}
EOF
}

echo "→ first sighting in the newer release"
REG_ISSUE="$(curl -fsS -X POST "${BASE}/v1/events" -H "Authorization: Bearer ${TOKEN}" \
    -H 'content-type: application/json' -d "$(reg_event 'reg@1.0.0+2')" | jq -r '.issueId')"
[[ -n "$REG_ISSUE" && "$REG_ISSUE" != "null" ]] || { echo "no issue for the regression probe" >&2; exit 1; }

echo "→ resolve it, anchored to the newer release"
curl -fsS -b "$JAR" -X POST "${BASE}/admin/api/issues/${REG_ISSUE}/resolve" \
    -H 'content-type: application/json' -d '{"release":"reg@1.0.0+2"}' >/dev/null

echo "→ the OLDER release crashing must not reopen it"
OLD="$(curl -fsS -X POST "${BASE}/v1/events" -H "Authorization: Bearer ${TOKEN}" \
    -H 'content-type: application/json' -d "$(reg_event 'reg@1.0.0+1')")"
[[ "$(echo "$OLD" | jq -r '.regressed')" == "false" ]] \
    || { echo "an older release reopened a fix: $OLD" >&2; exit 1; }
STATUS_NOW="$(curl -fsS -b "$JAR" "${BASE}/admin/api/issues/${REG_ISSUE}" | jq -r '.status')"
[[ "$STATUS_NOW" == "resolved" ]] \
    || { echo "issue left ${STATUS_NOW} after an older-release event, want resolved" >&2; exit 1; }

echo "→ the anchored release crashing must reopen it"
NEW="$(curl -fsS -X POST "${BASE}/v1/events" -H "Authorization: Bearer ${TOKEN}" \
    -H 'content-type: application/json' -d "$(reg_event 'reg@1.0.0+2')")"
[[ "$(echo "$NEW" | jq -r '.regressed')" == "true" ]] \
    || { echo "the fixed release crashed again and nothing reopened: $NEW" >&2; exit 1; }
REOPENED="$(curl -fsS -b "$JAR" "${BASE}/admin/api/issues/${REG_ISSUE}" | jq -r '.status + " " + (.regressedInRelease // "-")')"
[[ "$REOPENED" == "open reg@1.0.0+2" ]] \
    || { echo "reopened state is '${REOPENED}', want 'open reg@1.0.0+2'" >&2; exit 1; }

# ── symbolication, both directions ───────────────────────────────
#
# A stack the reader cannot read is the failure this product exists to
# prevent, and both halves of the path have broken in production: the
# resolver refused the source maps React Native actually produces, and
# a map uploaded after the crash rewrote nothing because no pass ever
# ran. Ad-hoc curl proved each fix once; this proves them every time.
echo "→ api-scope token"
API_TOKEN="$(curl -fsS -b "$JAR" -X POST "${BASE}/admin/api/projects/${PROJECT_ID}/tokens" \
    -H 'content-type: application/json' \
    -d '{"name":"e2e-ci","scope":"api"}' | jq -r '.token')"
[[ "$API_TOKEN" == st_* ]] || { echo "no api token: $API_TOKEN" >&2; exit 1; }

MAPDIR="$(mktemp -d)"
# One mapping: generated column 20 on line 1 → src/checkout.ts line 3,
# column 4. `oBAEI` is that segment in VLQ.
cat > "${MAPDIR}/index.android.bundle.map" <<'MAP'
{"version":3,"file":"index.android.bundle","sources":["src/checkout.ts"],
 "sourcesContent":["export function charge(userId) {\n  const token = mintToken(userId)\n  return post('/pay', { token })\n}\n"],
 "names":[],"mappings":"oBAEI"}
MAP

sym_event() {
    cat <<EOF
{"kind":"error","occurredAt":"2026-08-10T06:03:00Z","platform":"android",
 "release":"sym@1.0.0+1","environment":"test",
 "payload":{"error":{"type":"TypeError","message":"$1","stack":[
   {"file":"index.android.bundle","line":1,"column":20,"function":"e","inApp":true}]}}}
EOF
}

echo "→ a crash arrives before its map"
EARLY_ID="$(curl -fsS -X POST "${BASE}/v1/events" -H "Authorization: Bearer ${TOKEN}" \
    -H 'content-type: application/json' -d "$(sym_event 'before the map')" | jq -r '.eventId')"
EARLY_FILE="$(curl -fsS -b "$JAR" "${BASE}/admin/api/events/${EARLY_ID}" \
    | jq -r '.payload.error.stack[0].file')"
[[ "$EARLY_FILE" == "index.android.bundle" ]] \
    || { echo "expected the raw bundle path with no map on hand, got ${EARLY_FILE}" >&2; exit 1; }

echo "→ upload the map"
UPLOAD="$(curl -fsS -X POST "${BASE}/v1/releases/sym%401.0.0%2B1/artifacts" \
    -H "Authorization: Bearer ${API_TOKEN}" \
    -F kind=sourcemap -F "file=@${MAPDIR}/index.android.bundle.map")"
[[ "$(echo "$UPLOAD" | jq -r '.usable')" == "true" ]] \
    || { echo "the server could not parse a plain source map: $UPLOAD" >&2; exit 1; }

echo "→ retro pass rewrites the crash that predates it"
for i in $(seq 1 20); do
    LINE="$(curl -fsS -b "$JAR" "${BASE}/admin/api/events/${EARLY_ID}" \
        | jq -r '.payload.error.stack[0].contextLine // empty')"
    [[ -n "$LINE" ]] && break
    sleep 1
done
[[ "$LINE" == *"return post('/pay'"* ]] \
    || { echo "stored crash still unreadable after the upload: '${LINE}'" >&2; exit 1; }

echo "→ and the next crash resolves at ingest"
LATE_ID="$(curl -fsS -X POST "${BASE}/v1/events" -H "Authorization: Bearer ${TOKEN}" \
    -H 'content-type: application/json' -d "$(sym_event 'after the map')" | jq -r '.eventId')"
LATE_FILE="$(curl -fsS -b "$JAR" "${BASE}/admin/api/events/${LATE_ID}" \
    | jq -r '.payload.error.stack[0].file')"
[[ "$LATE_FILE" == "src/checkout.ts" ]] \
    || { echo "ingest-time symbolication did not run: ${LATE_FILE}" >&2; exit 1; }

echo "→ a bundle filed as a source map is refused a green light"
printf '\xc6\x1f\xbc\x03 not a map' > "${MAPDIR}/index.ios.bundle"
BAD="$(curl -fsS -X POST "${BASE}/v1/releases/sym%401.0.0%2B1/artifacts" \
    -H "Authorization: Bearer ${API_TOKEN}" \
    -F kind=sourcemap -F "file=@${MAPDIR}/index.ios.bundle")"
[[ "$(echo "$BAD" | jq -r '.usable')" == "false" ]] \
    || { echo "an unparseable artifact reported usable: $BAD" >&2; exit 1; }
# And it says what to upload instead. The CLI prints this verbatim at
# the moment of upload — for months it read `{ id }` off this response
# and dropped the rest, so the first anyone heard was a release page
# with the file sitting under a green light.
echo "$BAD" | jq -e '.hint | type == "string" and length > 0' > /dev/null \
    || { echo "a refused artifact came back with no hint: $BAD" >&2; exit 1; }
KINDS="$(curl -fsS -H "Authorization: Bearer ${API_TOKEN}" \
    "${BASE}/v1/releases/sym%401.0.0%2B1/artifacts" | jq -r '.kinds.sourcemap')"
[[ "$KINDS" == "1" ]] \
    || { echo "sourcemap count is ${KINDS}, want 1 — the unreadable one must not count" >&2; exit 1; }

# The two routes the dashboard's releases page actually calls. Every
# call to the list panicked on `ColumnNotFound("usable")` from server
# 2.15.0 to 2.21.1 — it read a column it had not selected, so the
# page got a dropped connection instead of a list. The one job that
# would have caught it triggers on `apps/rn-example/**`, so it did not
# run for six server releases. This is in the smoke because the smoke
# runs on all of them.
echo "→ the releases page has something to render"
RELEASES="$(curl -fsS -b "$JAR" "${BASE}/admin/api/projects/${PROJECT_ID}/releases")"
REL_ID="$(echo "$RELEASES" | jq -r '.releases[] | select(.name == "sym@1.0.0+1") | .id')"
[[ -n "$REL_ID" && "$REL_ID" != "null" ]] \
    || { echo "the uploaded release is missing from the list: $RELEASES" >&2; exit 1; }
[[ "$(echo "$RELEASES" | jq -r '.releases[] | select(.name == "sym@1.0.0+1") | .platforms | length')" != "0" ]] \
    || { echo "the release reports no platforms though events carry one: $RELEASES" >&2; exit 1; }

# Both artifacts, and `usable` telling them apart — this is where the
# flag lives, one per artifact rather than one per release.
ARTS="$(curl -fsS -b "$JAR" "${BASE}/admin/api/projects/${PROJECT_ID}/releases/${REL_ID}/artifacts")"
[[ "$(echo "$ARTS" | jq '[.artifacts[] | select(.usable == true)] | length')" == "1" ]] \
    || { echo "want exactly one usable artifact: $ARTS" >&2; exit 1; }
[[ "$(echo "$ARTS" | jq '[.artifacts[] | select(.usable == false)] | length')" == "1" ]] \
    || { echo "the unreadable artifact is not marked unusable: $ARTS" >&2; exit 1; }
rm -rf "$MAPDIR"

# ── the /api surface an agent drives ─────────────────────────────
#
# `sentori-cli issue list|bundle|note|resolve` and the MCP server all
# run on an api-scope token against these four routes. Nothing else
# exercises them, and "the agent surface" is a claim worth being able
# to check.
echo "→ /api/issues"
API_LIST="$(curl -fsS -H "Authorization: Bearer ${API_TOKEN}" "${BASE}/api/issues")"
[[ "$(echo "$API_LIST" | jq -r '.issues | length')" -ge 1 ]] \
    || { echo "/api/issues returned nothing: $API_LIST" >&2; exit 1; }

echo "→ the ingest token must not reach it"
ING_STATUS="$(curl -s -o /dev/null -w '%{http_code}' -H "Authorization: Bearer ${TOKEN}" \
    "${BASE}/api/issues")"
[[ "$ING_STATUS" == "403" ]] \
    || { echo "an app-embedded ingest token read the triage API: ${ING_STATUS}" >&2; exit 1; }

echo "→ issue bundle (markdown + json)"
BUNDLE_MD="$(curl -fsS -H "Authorization: Bearer ${API_TOKEN}" \
    "${BASE}/api/issues/${ISSUE_ID}/bundle")"
[[ "$BUNDLE_MD" == *"x is undefined"* ]] \
    || { echo "the bundle does not carry the error message" >&2; exit 1; }
BUNDLE_JSON="$(curl -fsS -H "Authorization: Bearer ${API_TOKEN}" \
    "${BASE}/api/issues/${ISSUE_ID}/bundle?format=json")"
echo "$BUNDLE_JSON" | jq -e '.issue' >/dev/null \
    || { echo "json bundle has no issue object: $(echo "$BUNDLE_JSON" | head -c 200)" >&2; exit 1; }

echo "→ note + resolve over the api token"
curl -fsS -X POST -H "Authorization: Bearer ${API_TOKEN}" -H 'content-type: application/json' \
    -d '{"body":"handled by the e2e"}' "${BASE}/api/issues/${ISSUE_ID}/notes" >/dev/null
curl -fsS -X POST -H "Authorization: Bearer ${API_TOKEN}" -H 'content-type: application/json' \
    -d '{"release":"e2e@1.0.0+1"}' "${BASE}/api/issues/${ISSUE_ID}/resolve" >/dev/null
API_STATUS="$(curl -fsS -b "$JAR" "${BASE}/admin/api/issues/${ISSUE_ID}" | jq -r '.status')"
[[ "$API_STATUS" == "resolved" ]] \
    || { echo "resolve over the api token left the issue ${API_STATUS}" >&2; exit 1; }

# ── attachments: the evidence path ───────────────────────────────
#
# Replays, screenshots and view trees ride a separate request from the
# event on purpose — the event is small and must land, the evidence is
# large and may not. Nothing exercised that request, and it is how the
# minute before a crash reaches the dashboard.
echo "→ attach a replay to the event"
ATT_DIR="$(mktemp -d)"
printf '{"t":-2.0,"mediaType":"image/svg+xml","base64":"PHN2Zy8+"}\n' > "${ATT_DIR}/screens.ndjson"
ATT="$(curl -fsS -X POST "${BASE}/v1/events/${EVENT_ID}/attachments/screens" \
    -H "Authorization: Bearer ${TOKEN}" \
    -F "source=js" -F "file=@${ATT_DIR}/screens.ndjson;type=application/x-ndjson")"
REF="$(echo "$ATT" | jq -r '.refId')"
[[ -n "$REF" && "$REF" != "null" ]] || { echo "no refId from the attachment upload: $ATT" >&2; exit 1; }

echo "→ the dashboard can read it back byte for byte"
BACK="$(curl -fsS -b "$JAR" "${BASE}/admin/api/attachments/${REF}")"
[[ "$BACK" == *'"mediaType":"image/svg+xml"'* ]] \
    || { echo "attachment came back different: $(echo "$BACK" | head -c 120)" >&2; exit 1; }

echo "→ the event lists it"
LISTED="$(curl -fsS -b "$JAR" "${BASE}/admin/api/events/${EVENT_ID}" \
    | jq -r '.attachments[] | select(.kind == "screens") | .ref')"
[[ "$LISTED" == "$REF" ]] \
    || { echo "the event does not list the attachment it was given: '${LISTED}'" >&2; exit 1; }

echo "→ an unknown kind is refused"
BAD_KIND="$(curl -s -o /dev/null -w '%{http_code}' -X POST \
    "${BASE}/v1/events/${EVENT_ID}/attachments/definitely-not-a-kind" \
    -H "Authorization: Bearer ${TOKEN}" -F "file=@${ATT_DIR}/screens.ndjson")"
[[ "$BAD_KIND" == "400" ]] \
    || { echo "an unknown attachment kind returned ${BAD_KIND}, want 400 — the database CHECK would refuse it anyway" >&2; exit 1; }
rm -rf "$ATT_DIR"

# ── push: the dashboard's half ───────────────────────────────────
#
# Push has been a complete backend since v0.2 with no dashboard, which
# is why it has no users: the only way to give Sentori an APNs key was
# an admin API nobody could see. These are the routes the new Settings
# ▸ Push tab drives.
echo "→ push health on a project that has never sent"
PH="$(curl -fsS -b "$JAR" "${BASE}/admin/api/projects/${PROJECT_ID}/push/health")"
[[ "$(echo "$PH" | jq -r '.sent24h')" == "0" && "$(echo "$PH" | jq -r '.liveTokens')" == "0" ]] \
    || { echo "a project with no push activity did not report zeros: $PH" >&2; exit 1; }
echo "$PH" | jq -e 'has("reasons") and has("quarantinedTokens")' >/dev/null \
    || { echo "health is missing the fields the card renders: $PH" >&2; exit 1; }

# A real EC key, so the credential this saves is one that could
# actually sign. The step used to post `{"keyId":"ABC123","teamId":
# "DEF456"}` — no topic, no key at all — and assert that it read
# back. It did read back. It could not have sent anything: the worker
# needs `topic` and a PEM, and a test that checks the row exists
# rather than that the thing works is how the form shipped for a year
# unable to accept a private key.
P8='-----BEGIN PRIVATE KEY-----
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgevZzL1gdAFr88hb2
OF/2NxApJCzGCEDdfSp6VQO30hyhRANCAAQRWz+jn65BtOMvdyHKcvjBeBSDZH2r
1RTwjmYSi9R/zpBnuQ4EiMnCqfMPWiZqB4QdbAd0E7oH50VpuZ1P087G
-----END PRIVATE KEY-----'

# A project with nothing set up has to say so, and say it in codes the
# console can translate rather than sentences it cannot.
echo "→ readiness names what is missing on an empty project"
RD="$(curl -fsS -b "$JAR" "${BASE}/admin/api/projects/${PROJECT_ID}/push/readiness")"
echo "$RD" | jq -e '.ready == false' > /dev/null \
    || { echo "an empty project reported ready: $RD" >&2; exit 1; }
echo "$RD" | jq -e '[.checks[] | select(.id == "no-device")] | length == 1' > /dev/null \
    || { echo "readiness did not notice there are no devices: $RD" >&2; exit 1; }
echo "$RD" | jq -e '[.checks[] | select(.data | type != "object")] | length == 0' > /dev/null \
    || { echo "a check carried something other than data: $RD" >&2; exit 1; }

echo "→ a key with its line breaks stripped is refused"
# What a single-line text field does to a pasted `.p8`, which is what
# the form used to be. The save succeeded and the key failed hours
# later on a device; now it fails here, with a sentence naming the
# problem.
FLAT="$(printf '%s' "$P8" | tr -d '\n')"
REFUSAL="$(jq -n --arg s "$FLAT" \
    '{provider:"apns",config:{keyId:"ABC1234567",teamId:"DEF7654321",topic:"com.example.app"},secret:$s}' \
    | curl -sS -b "$JAR" -X POST "${BASE}/admin/api/projects/${PROJECT_ID}/push/credentials" \
        -H 'content-type: application/json' --data @-)"
# A code and the field it is about, not a sentence: the console says
# it in the language the console is in, and this asserts the code
# rather than words that are no longer the server's to choose.
echo "$REFUSAL" | jq -e '.code == "pem-one-line" and .field == "secret"' > /dev/null \
    || { echo "a flattened key was accepted: $REFUSAL" >&2; exit 1; }

echo "→ a credential missing the field the worker reads is refused"
# The form's own placeholder said `bundleId`; the worker reads
# `topic`. Following the example exactly produced a credential that
# saved and then failed on the first send.
BAD="$(jq -n --arg s "$P8" \
    '{provider:"apns",config:{keyId:"ABC1234567",teamId:"DEF7654321",bundleId:"com.example.app"},secret:$s}' \
    | curl -sS -b "$JAR" -X POST "${BASE}/admin/api/projects/${PROJECT_ID}/push/credentials" \
        -H 'content-type: application/json' --data @-)"
echo "$BAD" | jq -e '.code == "field-missing" and .field == "topic"' > /dev/null \
    || { echo "bundleId was accepted in place of topic: $BAD" >&2; exit 1; }

echo "→ save and read back a provider credential"
jq -n --arg s "$P8" \
    '{provider:"apns",config:{keyId:"ABC1234567",teamId:"DEF7654321",topic:"com.example.app"},secret:$s}' \
    | curl -fsS -b "$JAR" -X POST "${BASE}/admin/api/projects/${PROJECT_ID}/push/credentials" \
        -H 'content-type: application/json' --data @- >/dev/null
CREDS="$(curl -fsS -b "$JAR" "${BASE}/admin/api/projects/${PROJECT_ID}/push/credentials")"
CRED="$(echo "$CREDS" | jq -r '.credentials[] | select(.kind == "apns") | .kind')"
[[ "$CRED" == "apns" ]] || { echo "the credential did not read back: '${CRED}'" >&2; exit 1; }
# And the list says it can be used, which is the part a dashboard
# shows and the part that was never true before.
echo "$CREDS" | jq -e '.credentials[] | select(.kind == "apns") | .problem == null' > /dev/null \
    || { echo "the saved credential reads back as unusable: $CREDS" >&2; exit 1; }

echo "→ a second credential of the same kind does not replace the first"
# This used to be an upsert. Pasting a key destroyed the working one
# in the statement that saved the new one, and both ways of holding
# the wrong file are invisible from the file: an App Store Connect
# .p8 is the same shape as an APNs .p8. So the new one is staged, and
# the one that sends keeps sending until somebody says otherwise.
FIRST_ID="$(echo "$CREDS" | jq -r '.credentials[] | select(.kind == "apns") | .id')"
SECOND="$(jq -n --arg s "$P8" \
    '{provider:"apns",config:{keyId:"ZZZ9999999",teamId:"DEF7654321",topic:"com.example.app"},secret:$s,label:"rotation"}' \
    | curl -fsS -b "$JAR" -X POST "${BASE}/admin/api/projects/${PROJECT_ID}/push/credentials" \
        -H 'content-type: application/json' --data @-)"
echo "$SECOND" | jq -e '.active == false' > /dev/null \
    || { echo "the second credential took over instead of staging: $SECOND" >&2; exit 1; }
SECOND_ID="$(echo "$SECOND" | jq -r '.id')"

AFTER="$(curl -fsS -b "$JAR" "${BASE}/admin/api/projects/${PROJECT_ID}/push/credentials")"
echo "$AFTER" | jq -e '[.credentials[] | select(.kind == "apns")] | length == 2' > /dev/null \
    || { echo "the second credential replaced the first: $AFTER" >&2; exit 1; }
# Exactly one sends. The partial unique index is the guarantee; this
# is the assertion that the guarantee is the one in force.
echo "$AFTER" | jq -e '[.credentials[] | select(.kind == "apns" and .active)] | length == 1' > /dev/null \
    || { echo "not exactly one active apns credential: $AFTER" >&2; exit 1; }
echo "$AFTER" | jq --arg id "$FIRST_ID" -e '.credentials[] | select(.id == $id) | .active == true' > /dev/null \
    || { echo "the credential that was sending stopped: $AFTER" >&2; exit 1; }

echo "→ the probe returns a verdict from the vendor's vocabulary"
# The credential is a locally-generated key Apple has never seen, so
# the honest answers are `rejected` (Apple said InvalidProviderToken,
# which is what it returned when this was checked by hand against
# api.push.apple.com) or `unreachable` (no egress from this runner).
# Asserting one of the two keeps a network-less CI from failing on a
# fact about Apple.
VERDICT="$(curl -fsS -b "$JAR" -X POST \
    "${BASE}/admin/api/projects/${PROJECT_ID}/push/credentials/${SECOND_ID}/probe" \
    -H 'content-type: application/json' -d '{}')"
echo "$VERDICT" | jq -e '.status == "rejected" or .status == "unreachable"' > /dev/null \
    || { echo "the probe said something else entirely: $VERDICT" >&2; exit 1; }
# Print which of the two, because they mean different things about the
# image: `rejected` is Apple answering, which means TLS trust roots
# work inside the container. reqwest 0.13.4 dropped the `webpki-roots`
# dependency for `rustls-platform-verifier`, and a distroless image
# with no OS trust store is exactly where that would show up — as
# `unreachable`, silently, on every outbound call the product makes.
echo "   probe verdict: $(echo "$VERDICT" | jq -r .status)"
# Whatever it said, it must have been written down — the column has
# held three legal values since 0007 and nothing had ever written one.
STORED="$(curl -fsS -b "$JAR" "${BASE}/admin/api/projects/${PROJECT_ID}/push/credentials" \
    | jq -r --arg id "$SECOND_ID" '.credentials[] | select(.id == $id) | .last_validate_status')"
[[ "$STORED" == "rejected" || "$STORED" == "unreachable" ]] \
    || { echo "the verdict was not stored: '${STORED}'" >&2; exit 1; }

echo "→ promoting a credential the vendor refused needs saying so twice"
if [[ "$STORED" == "rejected" ]]; then
    CODE="$(curl -sS -o /dev/null -w '%{http_code}' -b "$JAR" -X POST \
        "${BASE}/admin/api/projects/${PROJECT_ID}/push/credentials/${SECOND_ID}/activate" \
        -H 'content-type: application/json' -d '{}')"
    [[ "$CODE" == "409" ]] \
        || { echo "a refused credential was promoted on the first ask: ${CODE}" >&2; exit 1; }
fi

echo "→ and then it swaps, exactly one still sending"
curl -fsS -b "$JAR" -X POST \
    "${BASE}/admin/api/projects/${PROJECT_ID}/push/credentials/${SECOND_ID}/activate" \
    -H 'content-type: application/json' -d '{"force":true}' >/dev/null
SWAPPED="$(curl -fsS -b "$JAR" "${BASE}/admin/api/projects/${PROJECT_ID}/push/credentials")"
echo "$SWAPPED" | jq -e '[.credentials[] | select(.kind == "apns" and .active)] | length == 1' > /dev/null \
    || { echo "the swap left more or fewer than one active: $SWAPPED" >&2; exit 1; }
echo "$SWAPPED" | jq --arg id "$SECOND_ID" -e '.credentials[] | select(.id == $id) | .active == true' > /dev/null \
    || { echo "the promoted credential is not the one sending: $SWAPPED" >&2; exit 1; }

echo "→ delete addresses one row, not every credential of a kind"
curl -fsS -b "$JAR" -X DELETE \
    "${BASE}/admin/api/projects/${PROJECT_ID}/push/credentials/${SECOND_ID}" >/dev/null
LEFT="$(curl -fsS -b "$JAR" "${BASE}/admin/api/projects/${PROJECT_ID}/push/credentials")"
echo "$LEFT" | jq -e '[.credentials[] | select(.kind == "apns")] | length == 1' > /dev/null \
    || { echo "deleting one credential took the other: $LEFT" >&2; exit 1; }
# Put the survivor back in charge so the sends below have a
# credential: deleting the active one leaves the kind with none.
curl -fsS -b "$JAR" -X POST \
    "${BASE}/admin/api/projects/${PROJECT_ID}/push/credentials/${FIRST_ID}/activate" \
    -H 'content-type: application/json' -d '{}' >/dev/null

echo "→ register a device the way the SDK does"
IPT="$(curl -fsS -X POST "${BASE}/v1/push/devices" -H "Authorization: Bearer ${TOKEN}" \
    -H 'content-type: application/json' \
    -d '{"kind":"apns","env":"sandbox","nativeToken":"e2e-native-token-0001",
         "userKey":"a91f3c02deadbeefa91f3c02deadbeef",
         "metadata":{"appVersion":"1.4.0","channel":"e2e"}}' \
    | jq -r '.spToken')"
[[ -n "$IPT" && "$IPT" != "null" ]] || { echo "device registration returned no token id" >&2; exit 1; }
LIVE="$(curl -fsS -b "$JAR" "${BASE}/admin/api/projects/${PROJECT_ID}/push/health" | jq -r '.liveTokens')"
[[ "$LIVE" == "1" ]] || { echo "health says ${LIVE} live devices after one registration" >&2; exit 1; }

# `metadata` was an advertised SDK option that reached neither the
# request body nor the server struct, while the column it names has
# existed since the table was created. Every device row read '{}' and
# the integrator who passed it had no way to find out (insight,
# 2026-08-11). The device list is where they check; assert it carries
# what was sent.
echo "→ what the device reported about itself comes back"
DEV="$(curl -fsS -b "$JAR" "${BASE}/admin/api/projects/${PROJECT_ID}/push/devices")"
[[ "$(echo "$DEV" | jq -r '.devices[0].metadata.appVersion')" == "1.4.0" ]] \
    || { echo "metadata did not survive registration: $DEV" >&2; exit 1; }
[[ "$(echo "$DEV" | jq -r '.devices[0].addressable')" == "true" ]] \
    || { echo "a device registered with a userKey is not addressable: $DEV" >&2; exit 1; }
# The token itself must not come back — a push token is a capability,
# and the row exists to say which device, not to hand one out.
[[ "$(echo "$DEV" | jq -r '.devices[0] | has("nativeToken")')" == "false" ]] \
    || { echo "the device list returned a usable push token" >&2; exit 1; }

echo "→ the device carries the same identity the events do"
# The join S3 is built on: a device addressable by the user key an
# event already carried. If these two columns ever stop matching in
# type or value, "notify the people who hit this issue" quietly
# reaches nobody.
LINKED="$(curl -fsS -b "$JAR" "${BASE}/admin/api/projects/${PROJECT_ID}/push/health" | jq -r '.identifiedTokens')"
[[ "$LINKED" == "1" ]] \
    || { echo "the registered device is not addressable by user key: ${LINKED}" >&2; exit 1; }

# Revoking and coming back. Neither had ever been exercised until
# insight ran them by hand: the revoke endpoint was deleting from a
# table nothing reads, so a device kept receiving after `unregister`
# reported success. And a revoked row is revived by the next
# registration, which is intended — but it used to carry the
# quarantine reason back with it, so the device list described a live
# device with the words of the failure that killed its old token.
echo "→ revoking a device takes it out of the send set"
curl -fsS -X DELETE "${BASE}/v1/push/devices/${IPT}" -H "Authorization: Bearer ${TOKEN}" \
    | jq -e '.status == "revoked"' > /dev/null \
    || { echo "revoke did not report revoking" >&2; exit 1; }
LIVE="$(curl -fsS -b "$JAR" "${BASE}/admin/api/projects/${PROJECT_ID}/push/health" | jq -r '.liveTokens')"
[[ "$LIVE" == "0" ]] \
    || { echo "health says ${LIVE} live devices after revoking the only one" >&2; exit 1; }

echo "→ revoking twice says so the second time"
curl -fsS -X DELETE "${BASE}/v1/push/devices/${IPT}" -H "Authorization: Bearer ${TOKEN}" \
    | jq -e '.status == "not_found"' > /dev/null \
    || { echo "a revoke that changed nothing reported success" >&2; exit 1; }

echo "→ registering again brings it back, without the old failure attached"
IPT2="$(curl -fsS -X POST "${BASE}/v1/push/devices" -H "Authorization: Bearer ${TOKEN}" \
    -H 'content-type: application/json' \
    -d '{"kind":"apns","env":"sandbox","nativeToken":"e2e-native-token-0001",
         "userKey":"a91f3c02deadbeefa91f3c02deadbeef"}' \
    | jq -r '.spToken')"
[[ "$IPT2" == "$IPT" ]] \
    || { echo "the same native token produced a different row: ${IPT2} vs ${IPT}" >&2; exit 1; }
DEV="$(curl -fsS -b "$JAR" "${BASE}/admin/api/projects/${PROJECT_ID}/push/devices")"
# This row was revoked by the DELETE, not by quarantine, so it never
# had a `quarantine_reason` to lose — insight pointed out that its
# absence here proves nothing, and they are right. The assertion
# stays as a floor (the revival must not *invent* one) and the real
# coverage of the strip is the unit test on the classifier, which is
# what decides whether a reason is ever written.
[[ "$(echo "$DEV" | jq -r '.devices[0].metadata | has("quarantine_reason")')" == "false" ]] \
    || { echo "a revived device carries a quarantine reason it never had: $DEV" >&2; exit 1; }
[[ "$(echo "$DEV" | jq -r '.devices[0].metadata | has("revived_at")')" == "true" ]] \
    || { echo "a revived device does not say it was revived: $DEV" >&2; exit 1; }
[[ "$(echo "$DEV" | jq -r '.devices[0].metadata.appVersion')" == "1.4.0" ]] \
    || { echo "reviving lost what the device had reported: $DEV" >&2; exit 1; }

# The address has to survive the vendor rotating its token. It did
# not: the row was unique on (project, provider, native_token), so a
# new token wrote a new row with a new id, and every backend holding
# the old one was addressing nothing — with nothing to tell it.
echo "→ a rotated token keeps the same spToken"
SP1="$(curl -fsS -X POST "${BASE}/v1/push/devices" -H "Authorization: Bearer ${TOKEN}" \
    -H 'content-type: application/json' \
    -d '{"kind":"apns","env":"sandbox","nativeToken":"e2e-rotate-before",
         "installId":"e2e-install-0001"}' | jq -r '.spToken')"
[[ -n "$SP1" && "$SP1" != "null" ]] || { echo "no spToken in the response" >&2; exit 1; }

SP2="$(curl -fsS -X POST "${BASE}/v1/push/devices" -H "Authorization: Bearer ${TOKEN}" \
    -H 'content-type: application/json' \
    -d '{"kind":"apns","env":"sandbox","nativeToken":"e2e-rotate-after",
         "installId":"e2e-install-0001"}' | jq -r '.spToken')"
[[ "$SP1" == "$SP2" ]] \
    || { echo "the address changed when the token rotated: ${SP1} → ${SP2}" >&2; exit 1; }

# And it is one device afterwards, not two.
ROTATED="$(curl -fsS -b "$JAR" "${BASE}/admin/api/projects/${PROJECT_ID}/push/devices?limit=100" \
    | jq '[.devices[] | select(.id == "'"$SP1"'")] | length')"
[[ "$ROTATED" == "1" ]] || { echo "the rotated device is not a single row" >&2; exit 1; }

# A device that registered before install ids existed still upserts
# the way it always did, and picks one up when it next registers.
echo "→ a device with no installId still registers"
SPOLD="$(curl -fsS -X POST "${BASE}/v1/push/devices" -H "Authorization: Bearer ${TOKEN}" \
    -H 'content-type: application/json' \
    -d '{"kind":"apns","env":"sandbox","nativeToken":"e2e-legacy-token"}' | jq -r '.spToken')"
SPADOPT="$(curl -fsS -X POST "${BASE}/v1/push/devices" -H "Authorization: Bearer ${TOKEN}" \
    -H 'content-type: application/json' \
    -d '{"kind":"apns","env":"sandbox","nativeToken":"e2e-legacy-token",
         "installId":"e2e-install-adopt"}' | jq -r '.spToken')"
[[ "$SPOLD" == "$SPADOPT" ]] \
    || { echo "adopting an install id moved the device: ${SPOLD} → ${SPADOPT}" >&2; exit 1; }

# `spTokens` is the name a caller sends to; `tokenIds` still works.
echo "→ a send accepts spTokens"
curl -fsS -X POST "${BASE}/v1/push/sends" -H "Authorization: Bearer ${API_TOKEN}" \
    -H 'content-type: application/json' \
    -d '{"spTokens":["'"$SP1"'"],"payload":{"title":"e2e","body":"spTokens"}}' \
    | jq -e '.queued == 1' > /dev/null \
    || { echo "a send addressed by spTokens queued nothing" >&2; exit 1; }

echo "→ a send actually queues"
# `POST /v1/push/sends` had ten values for nine columns and answered
# 500 to everything. An endpoint that always 500s is
# indistinguishable from a feature nobody uses, which is how this
# read for a year.
SEND="$(curl -fsS -X POST "${BASE}/v1/push/sends" -H "Authorization: Bearer ${API_TOKEN}" \
    -H 'content-type: application/json' \
    -d "{\"tokenIds\":[\"${IPT}\"],\"payload\":{\"title\":\"e2e\",\"body\":\"hello\"}}")"
echo "$SEND" | jq -e '.queued >= 1 and (.sendId | length == 36)' >/dev/null \
    || { echo "send queued nothing: $SEND" >&2; exit 1; }
QUEUED="$(curl -fsS -b "$JAR" "${BASE}/admin/api/projects/${PROJECT_ID}/push/sends" \
    | jq -r '.sends | length')"
[[ "$QUEUED" -ge 1 ]] \
    || { echo "the dashboard's send list is empty after a send" >&2; exit 1; }


# ── push: who a send is for ──────────────────────────────────────
#
# The audience engine's SQL had never run against a database. Every
# assertion below is one the unit tests structurally cannot make:
# they check that a fragment is *shaped* right, and these check that
# Postgres agrees about what it selects.

echo "→ a send with no target is refused rather than sent to everyone"
NOTGT="$(curl -sS -o /dev/null -w '%{http_code}' -X POST "${BASE}/v1/push/sends" \
    -H "Authorization: Bearer ${API_TOKEN}" -H 'content-type: application/json' \
    -d '{"payload":{"title":"e2e"}}')"
[[ "$NOTGT" == "400" ]] \
    || { echo "a send naming no target answered ${NOTGT}, not 400" >&2; exit 1; }

# The hash is computed here by sha256sum, not by our own code. If the
# server's hash and the SDK's ever drift, one of them stops agreeing
# with this line — which is the point of computing it outside.
UKEY="$(printf 'usr_e2e_alice' | shasum -a 256 | cut -d' ' -f1)"

curl -fsS -X POST "${BASE}/v1/push/devices" -H "Authorization: Bearer ${TOKEN}" \
    -H 'content-type: application/json' \
    -d '{"kind":"apns","env":"sandbox","nativeToken":"e2e-aud-alice",
         "installId":"e2e-aud-alice","userKey":"'"$UKEY"'",
         "traits":{"plan":"pro","locale":"ja-JP","e2e":"aud"},
         "metadata":{"appVersion":"4.10.0"}}' >/dev/null

curl -fsS -X POST "${BASE}/v1/push/devices" -H "Authorization: Bearer ${TOKEN}" \
    -H 'content-type: application/json' \
    -d '{"kind":"apns","env":"sandbox","nativeToken":"e2e-aud-bob",
         "installId":"e2e-aud-bob",
         "traits":{"plan":"free","locale":"ja-JP","e2e":"aud"},
         "metadata":{"appVersion":"4.2.0"}}' >/dev/null

curl -fsS -X POST "${BASE}/v1/push/devices" -H "Authorization: Bearer ${TOKEN}" \
    -H 'content-type: application/json' \
    -d '{"kind":"apns","env":"sandbox","nativeToken":"e2e-aud-carol",
         "installId":"e2e-aud-carol",
         "traits":{"plan":"pro","locale":"en-US","e2e":"aud"},
         "metadata":{"appVersion":"nightly"}}' >/dev/null

send_count() {
    curl -fsS -X POST "${BASE}/v1/push/sends" -H "Authorization: Bearer ${API_TOKEN}" \
        -H 'content-type: application/json' -d "$1" | jq -r '.queued'
}

echo "→ appUserId reaches the device that registered under it, and only that one"
N="$(send_count '{"appUserId":"usr_e2e_alice","payload":{"title":"t1"}}')"
[[ "$N" == "1" ]] \
    || { echo "targeting by appUserId reached ${N} devices, not 1" >&2; exit 1; }

echo "→ an appUserId nobody registered reaches nobody"
N="$(send_count '{"appUserId":"usr_e2e_nobody","payload":{"title":"t2"}}')"
[[ "$N" == "0" ]] \
    || { echo "an unknown appUserId reached ${N} devices" >&2; exit 1; }

echo "→ traits select on the person, not on the device"
N="$(send_count '{"traits":{"plan":"pro","e2e":"aud"},"payload":{"title":"t3"}}')"
[[ "$N" == "2" ]] \
    || { echo "traits {plan:pro} reached ${N} devices, not 2" >&2; exit 1; }
N="$(send_count '{"traits":{"plan":"pro","locale":"ja-JP","e2e":"aud"},
                       "payload":{"title":"t4"}}')"
[[ "$N" == "1" ]] \
    || { echo "two traits reached ${N} devices, not 1" >&2; exit 1; }

# The assertion this whole file exists for. Compared as text, "4.10.0"
# sorts *below* "4.2", so a text comparison answers 1 here and does it
# for the first time on the day a project ships its tenth minor
# release — in production, aimed at the users a fix was meant for.
echo "→ 4.10.0 is a later version than 4.2, not an earlier one"
N="$(send_count '{"audience":{"all":[{"trait":"e2e","is":"aud"},
                    {"device":"appVersion","versionGte":"4.2"}]},
                  "payload":{"title":"t5"}}')"
[[ "$N" == "2" ]] \
    || { echo "versionGte 4.2 reached ${N} devices, not 2 (4.10.0 and 4.2.0)" >&2; exit 1; }

echo "→ 4.2 and 4.2.0 are the same version"
N="$(send_count '{"audience":{"all":[{"trait":"e2e","is":"aud"},
                    {"device":"appVersion","versionLte":"4.2"}]},
                  "payload":{"title":"t6"}}')"
[[ "$N" == "1" ]] \
    || { echo "versionLte 4.2 reached ${N} devices, not 1" >&2; exit 1; }

echo "→ a version nothing can parse is left out, not swept in"
N="$(send_count '{"audience":{"all":[{"trait":"e2e","is":"aud"},
                    {"device":"appVersion","versionGte":"0.0.1"}]},
                  "payload":{"title":"t7"}}')"
[[ "$N" == "2" ]] \
    || { echo "a device reporting \"nightly\" was included by a version test: ${N}" >&2; exit 1; }

echo "→ the whole expression: and, or, in, not"
N="$(send_count '{"audience":{"all":[
        {"trait":"e2e","is":"aud"},
        {"trait":"plan","in":["pro","team"]},
        {"device":"appVersion","versionGte":"4.2"},
        {"any":[{"trait":"locale","is":"ja-JP"},{"trait":"org","is":"acme"}]},
        {"not":{"trait":"churned","is":true}}]},
      "payload":{"title":"t8"}}')"
[[ "$N" == "1" ]] \
    || { echo "the worked example reached ${N} devices, not 1" >&2; exit 1; }

# A negation must keep the rows that have no such key at all —
# otherwise "not churned" quietly means "known not to be churned",
# and everyone who never had the trait drops out.
echo "→ not on a trait nobody has keeps everybody"
N="$(send_count '{"audience":{"all":[{"trait":"e2e","is":"aud"},
                    {"not":{"trait":"churned","is":true}}]},
                  "payload":{"title":"t9"}}')"
[[ "$N" == "3" ]] \
    || { echo "a negation on an absent trait dropped devices: ${N}" >&2; exit 1; }

echo "→ an empty or-group matches nothing rather than everything"
N="$(send_count '{"audience":{"any":[]},"payload":{"title":"t10"}}')"
[[ "$N" == "0" ]] \
    || { echo "an empty any reached ${N} devices" >&2; exit 1; }

echo "→ two targeting modes at once is refused, not guessed at"
BOTH="$(curl -sS -o /dev/null -w '%{http_code}' -X POST "${BASE}/v1/push/sends" \
    -H "Authorization: Bearer ${API_TOKEN}" -H 'content-type: application/json' \
    -d '{"appUserId":"usr_e2e_alice","traits":{"plan":"pro"},"payload":{}}')"
[[ "$BOTH" == "400" ]] \
    || { echo "appUserId together with traits answered ${BOTH}, not 400" >&2; exit 1; }

# A preview that is an estimate is a preview nobody can act on: the
# only other way to find out what an expression matches is to send to
# it, and that cannot be undone.
echo "→ the preview counts exactly what a send would reach"
AUD='{"all":[{"trait":"e2e","is":"aud"},{"trait":"plan","in":["pro","team"]},
              {"device":"appVersion","versionGte":"4.2"}]}'
PREV="$(curl -fsS -b "$JAR" -X POST \
    "${BASE}/admin/api/projects/${PROJECT_ID}/push/audience/preview" \
    -H 'content-type: application/json' -d "{\"audience\":${AUD}}")"
MATCHED="$(echo "$PREV" | jq -r '.matched')"
SENT="$(send_count "{\"audience\":${AUD},\"payload\":{\"title\":\"t12\"}}")"
[[ "$MATCHED" == "$SENT" ]] \
    || { echo "the preview said ${MATCHED} and the send reached ${SENT}" >&2; exit 1; }
[[ "$MATCHED" -ge 1 ]] \
    || { echo "the preview and the send agreed on nothing, which agrees on nothing" >&2; exit 1; }

# A preview is not a way around what the device list refuses to show.
echo "→ the preview does not hand back a push token"
echo "$PREV" | jq -e '[.sample[] | select(has("native_token") or has("nativeToken"))] | length == 0' \
    > /dev/null || { echo "the preview returned a usable push token: $PREV" >&2; exit 1; }
echo "$PREV" | jq -e '.sample[0].traits.plan != null' > /dev/null \
    || { echo "the sample says nothing about why it matched: $PREV" >&2; exit 1; }

# The console can send to an audience, and only to the one the
# operator was looking at. Devices register between reading a number
# and pressing a button, and nothing here can be undone.
echo "→ the console will not send to a number nobody read"
STALE="$(curl -sS -o /dev/null -w '%{http_code}' -b "$JAR" -X POST \
    "${BASE}/admin/api/projects/${PROJECT_ID}/push/audience/send" \
    -H 'content-type: application/json' \
    -d "{\"audience\":${AUD},\"title\":\"e2e\",\"expectedMatched\":99}")"
[[ "$STALE" == "409" ]] \
    || { echo "a send against a stale count answered ${STALE}, not 409" >&2; exit 1; }

echo "→ and sends to exactly that number when it still holds"
CONSOLE="$(curl -fsS -b "$JAR" -X POST \
    "${BASE}/admin/api/projects/${PROJECT_ID}/push/audience/send" \
    -H 'content-type: application/json' \
    -d "{\"audience\":${AUD},\"title\":\"e2e\",\"body\":\"console\",
         \"expectedMatched\":${MATCHED}}" | jq -r '.queued')"
[[ "$CONSOLE" == "$MATCHED" ]] \
    || { echo "the console queued ${CONSOLE} for an audience of ${MATCHED}" >&2; exit 1; }

# The count guard does not catch a double press: sending does not
# change the audience, so the second press finds the same number and
# passes. A key does.
# Counting the rows says nothing about what is in them. This send
# went out with a title, and the queued row has to carry that title —
# not whatever else happened to be bound at that position.
echo "→ the console's send carries the message, not something else"
curl -fsS -b "$JAR" -X POST \
    "${BASE}/admin/api/projects/${PROJECT_ID}/push/audience/send" \
    -H 'content-type: application/json' \
    -d "{\"audience\":${AUD},\"title\":\"console-payload-probe\",
         \"body\":\"hello\",\"expectedMatched\":${MATCHED}}" > /dev/null
curl -fsS -b "$JAR" "${BASE}/admin/api/projects/${PROJECT_ID}/push/sends?limit=50" \
    | jq -e '[.sends[] | select(.payload.title == "console-payload-probe")] | length >= 1' \
    > /dev/null \
    || { echo "the console's send did not carry its own title" >&2; exit 1; }

echo "→ pressing the console's send twice queues once"
KEY="e2e-console-$RANDOM"
Q1="$(curl -fsS -b "$JAR" -X POST \
    "${BASE}/admin/api/projects/${PROJECT_ID}/push/audience/send" \
    -H 'content-type: application/json' \
    -d "{\"audience\":${AUD},\"title\":\"twice\",
         \"idempotencyKey\":\"${KEY}\",\"expectedMatched\":${MATCHED}}")"
[[ "$(echo "$Q1" | jq -r '.queued')" == "$MATCHED" ]] \
    || { echo "the first press queued $(echo "$Q1" | jq -r '.queued')" >&2; exit 1; }

Q2="$(curl -fsS -b "$JAR" -X POST \
    "${BASE}/admin/api/projects/${PROJECT_ID}/push/audience/send" \
    -H 'content-type: application/json' \
    -d "{\"audience\":${AUD},\"title\":\"twice\",
         \"idempotencyKey\":\"${KEY}\",\"expectedMatched\":${MATCHED}}")"
echo "$Q2" | jq -e '.queued == 0 and .alreadySent == true' > /dev/null \
    || { echo "the second press sent again: $Q2" >&2; exit 1; }

echo "→ a notification with no title is refused"
NOTITLE="$(curl -sS -o /dev/null -w '%{http_code}' -b "$JAR" -X POST \
    "${BASE}/admin/api/projects/${PROJECT_ID}/push/audience/send" \
    -H 'content-type: application/json' \
    -d "{\"audience\":${AUD},\"title\":\"  \",\"expectedMatched\":${MATCHED}}")"
[[ "$NOTITLE" == "400" ]] \
    || { echo "a titleless notification answered ${NOTITLE}, not 400" >&2; exit 1; }

# "Notify the people who hit this issue" is the thing the whole design
# points at, and the join had never run. `issue_user_hits` is written
# at ingest and carries the same hash the device row does.
# The shape a backend actually writes: it computed the list from its
# own database, and still wants Sentori to apply the conditions only
# Sentori knows. The three shorthands cannot be combined, which reads
# as if this were impossible — the restriction is on the sugar, not on
# the expression.
# `idempotencyKey` is documented as a dedup key. The unique index
# behind it is on (project_id, idempotency_key) — but one send to N
# devices writes N rows carrying the same key, so the second row
# collides with the first and the whole statement is refused. The
# field only ever worked for an audience of exactly one.
echo "→ a keyed send reaches everyone it matched, not just the first"
KEYED='{"audience":{"all":[{"trait":"e2e","is":"aud"},{"trait":"plan","is":"pro"}]},
        "idempotencyKey":"e2e-idem-1","payload":{"title":"idem"}}'
N="$(send_count "$KEYED")"
[[ "$N" == "2" ]] \
    || { echo "a keyed send to 2 devices queued ${N}" >&2; exit 1; }

# And sending it again with the same key adds nothing, which is what
# the word means.
echo "→ the same key twice queues nothing the second time"
N="$(send_count "$KEYED")"
[[ "$N" == "0" ]] \
    || { echo "the same idempotency key queued ${N} more" >&2; exit 1; }

# A different key is a different send.
echo "→ a different key is a different send"
N="$(send_count "${KEYED/e2e-idem-1/e2e-idem-2}")"
[[ "$N" == "2" ]] \
    || { echo "a fresh key queued ${N}, not 2" >&2; exit 1; }

# The console has counted before sending since audiences existed. A
# backend had no way to — the preview is behind a browser session — so
# the one caller that sends automatically was the one that could not
# find out how large a condition was first.
# And by now the project is set up — a credential, devices, identities,
# traits, metadata. A checklist that always has something on it is a
# checklist nobody reads, so this asserts it goes quiet.
echo "→ readiness goes quiet once the project is actually set up"
RD="$(curl -fsS -b "$JAR" "${BASE}/admin/api/projects/${PROJECT_ID}/push/readiness")"
echo "$RD" | jq -e '.ready == true' > /dev/null \
    || { echo "a set-up project still reported blocked: $RD" >&2; exit 1; }
echo "$RD" | jq -e '[.checks[] | select(.level == "blocked" or .level == "warn")] | length == 0' \
    > /dev/null || { echo "a set-up project still has gaps: $RD" >&2; exit 1; }
echo "$RD" | jq -e '.live > 0' > /dev/null \
    || { echo "readiness counted no devices: $RD" >&2; exit 1; }

# The snippets the console hands a backend, run as a backend would run
# them. A snippet is documentation somebody compiles, and the last time
# this repo shipped a command it had not run, the command POSTed to a
# route no server had ever served.
# The curl on the getting-started page, executed exactly as printed.
# It sent `timestamp` instead of `occurredAt` until 2026-08-27 and had
# answered 422 for as long as the file existed — the first request a
# reader makes, and the first one an agent copies.
echo "→ the curl on the getting-started page answers 202"
SENTORI_BASE="${BASE}" SENTORI_TOKEN="${TOKEN}" \
    node "${ROOT}/../scripts/check-docs-curl.mjs" \
    || { echo "the curl the docs print does not work" >&2; exit 1; }

echo "→ the Node and Python snippets the console hands out actually send"
SENTORI_BASE="${BASE}" SENTORI_API_TOKEN="${API_TOKEN}" \
    node "${ROOT}/../scripts/check-push-snippets.mjs" \
    || { echo "a snippet the console hands out does not work" >&2; exit 1; }

# One call, one id, one poll. The caller used to be handed one id per
# device and had to poll each — and for a large send the response was
# megabytes of uuid before it was anything else.
echo "→ a send answers with one id for the call"
BATCH="$(curl -fsS -X POST "${BASE}/v1/push/sends" -H "Authorization: Bearer ${API_TOKEN}" \
    -H 'content-type: application/json' \
    -d '{"traits":{"e2e":"aud"},"payload":{"title":"batch"}}')"
SID="$(echo "$BATCH" | jq -r '.sendId')"
[[ "$SID" != "null" && -n "$SID" ]] \
    || { echo "no sendId came back: $BATCH" >&2; exit 1; }
NDEV="$(echo "$BATCH" | jq -r '.queued')"

echo "→ the id answers what happened to the whole call"
SUM="$(curl -fsS "${BASE}/v1/push/sends/${SID}" -H "Authorization: Bearer ${API_TOKEN}")"
echo "$SUM" | jq -e --argjson n "$NDEV" '.counts.total == $n' > /dev/null \
    || { echo "the summary does not count what the send queued: $SUM" >&2; exit 1; }
echo "$SUM" | jq -e '.counts | (.queued + .sent + .failed) == .total' > /dev/null \
    || { echo "the counts do not add up to the total: $SUM" >&2; exit 1; }
# `delivered` is what a device reported, and no device reported here.
echo "$SUM" | jq -e '.counts.delivered == 0 and (.state | test("in_flight|done"))' > /dev/null \
    || { echo "the summary is not a state and a count: $SUM" >&2; exit 1; }

echo "→ and the rows behind it, one per device"
DEL="$(curl -fsS "${BASE}/v1/push/sends/${SID}/deliveries" -H "Authorization: Bearer ${API_TOKEN}")"
echo "$DEL" | jq -e --argjson n "$NDEV" '(.deliveries | length) == $n' > /dev/null \
    || { echo "the deliveries do not match the count: $DEL" >&2; exit 1; }
echo "$DEL" | jq -e '.deliveries[0] | has("spToken") and has("status") and has("deliveredAt")' \
    > /dev/null || { echo "a delivery row says too little: $DEL" >&2; exit 1; }

echo "→ the listing pages by cursor, not by offset"
P1="$(curl -fsS "${BASE}/v1/push/sends/${SID}/deliveries?limit=1" \
    -H "Authorization: Bearer ${API_TOKEN}")"
CUR="$(echo "$P1" | jq -r '.nextCursor')"
[[ "$CUR" != "null" ]] || { echo "a full page handed back no cursor: $P1" >&2; exit 1; }
P2="$(curl -fsS "${BASE}/v1/push/sends/${SID}/deliveries?limit=1&cursor=${CUR}" \
    -H "Authorization: Bearer ${API_TOKEN}")"
[[ "$(echo "$P1" | jq -r '.deliveries[0].deliveryId')" \
   != "$(echo "$P2" | jq -r '.deliveries[0].deliveryId')" ]] \
    || { echo "the cursor did not move: $P2" >&2; exit 1; }

echo "→ an id nobody minted is a 404, not a 200 saying so"
MISS="$(curl -sS -o /dev/null -w '%{http_code}' \
    "${BASE}/v1/push/sends/019ff080-2aeb-7e30-aba1-4431b296dfff" \
    -H "Authorization: Bearer ${API_TOKEN}")"
[[ "$MISS" == "404" ]] || { echo "an unknown sendId answered ${MISS}" >&2; exit 1; }

echo "→ the app's own token cannot read what was sent to whom"
FORB="$(curl -sS -o /dev/null -w '%{http_code}' "${BASE}/v1/push/sends/${SID}" \
    -H "Authorization: Bearer ${TOKEN}")"
[[ "$FORB" == "403" ]] || { echo "an ingest token read a send, answering ${FORB}" >&2; exit 1; }

echo "→ a backend can count an audience without sending to it"
CNT="$(curl -fsS -X POST "${BASE}/v1/push/audience/count" -H "Authorization: Bearer ${API_TOKEN}" \
    -H 'content-type: application/json' \
    -d '{"traits":{"plan":"pro","e2e":"aud"}}' | jq -r '.matched')"
SENT="$(send_count '{"traits":{"plan":"pro","e2e":"aud"},"payload":{"title":"c"}}')"
[[ "$CNT" == "$SENT" ]] \
    || { echo "count said ${CNT} and the send reached ${SENT}" >&2; exit 1; }
[[ "$CNT" -ge 1 ]] || { echo "count and send agreed on nothing" >&2; exit 1; }

echo "→ counting does not send"
BEFORE="$(curl -fsS -b "$JAR" "${BASE}/admin/api/projects/${PROJECT_ID}/push/sends?limit=1" \
    | jq -r '.total')"
curl -fsS -X POST "${BASE}/v1/push/audience/count" -H "Authorization: Bearer ${API_TOKEN}" \
    -H 'content-type: application/json' -d '{"traits":{"e2e":"aud"}}' > /dev/null
AFTER="$(curl -fsS -b "$JAR" "${BASE}/admin/api/projects/${PROJECT_ID}/push/sends?limit=1" \
    | jq -r '.total')"
[[ "$BEFORE" == "$AFTER" ]] \
    || { echo "counting queued something: ${BEFORE} → ${AFTER}" >&2; exit 1; }

echo "→ the app's own token cannot count the customer's users"
FORB="$(curl -sS -o /dev/null -w '%{http_code}' -X POST "${BASE}/v1/push/audience/count" \
    -H "Authorization: Bearer ${TOKEN}" -H 'content-type: application/json' \
    -d '{"traits":{"e2e":"aud"}}')"
[[ "$FORB" == "403" ]] \
    || { echo "an ingest token counted, answering ${FORB}" >&2; exit 1; }

echo "→ a backend's list can be narrowed by a device condition"
N="$(send_count '{"audience":{"all":[
        {"any":[{"user":"usr_e2e_alice"},{"user":"usr_e2e_nobody"}]},
        {"device":"appVersion","versionGte":"4.2"}]},
      "payload":{"title":"mixed"}}')"
[[ "$N" == "1" ]] \
    || { echo "a list narrowed by a version reached ${N} devices, not 1" >&2; exit 1; }

# And the condition really is applied, rather than the list winning.
N="$(send_count '{"audience":{"all":[
        {"any":[{"user":"usr_e2e_alice"},{"user":"usr_e2e_nobody"}]},
        {"device":"appVersion","versionGte":"9.0"}]},
      "payload":{"title":"mixed"}}')"
[[ "$N" == "0" ]] \
    || { echo "the device condition was ignored: ${N} devices" >&2; exit 1; }

echo "→ an issue selects the people it happened to"
HITKEY="$(printf 'usr_e2e_hit' | shasum -a 256 | cut -d' ' -f1)"
curl -fsS -X POST "${BASE}/v1/events" -H "Authorization: Bearer ${TOKEN}" \
    -H 'content-type: application/json' \
    -d '{"id":"019fe900-0000-7000-8000-0000000e2e77","kind":"error",
         "occurredAt":"2026-08-10T06:05:00Z","platform":"javascript",
         "release":"e2e@1.0.0+1","environment":"test",
         "userKey":"'"$HITKEY"'",
         "payload":{"error":{"type":"IssueAudience","message":"hit","stack":[]}}}' \
    > /tmp/hit.json
HITISSUE="$(jq -r '.issueId' /tmp/hit.json)"
[[ -n "$HITISSUE" && "$HITISSUE" != "null" ]] \
    || { echo "no issue for the identified event: $(cat /tmp/hit.json)" >&2; exit 1; }

# Two devices: one held by the person who hit it, one not.
curl -fsS -X POST "${BASE}/v1/push/devices" -H "Authorization: Bearer ${TOKEN}" \
    -H 'content-type: application/json' \
    -d '{"kind":"apns","env":"sandbox","nativeToken":"e2e-issue-hit",
         "installId":"e2e-issue-hit","userKey":"'"$HITKEY"'"}' >/dev/null
curl -fsS -X POST "${BASE}/v1/push/devices" -H "Authorization: Bearer ${TOKEN}" \
    -H 'content-type: application/json' \
    -d '{"kind":"apns","env":"sandbox","nativeToken":"e2e-issue-miss",
         "installId":"e2e-issue-miss","userKey":"'"$(printf 'usr_e2e_other' | shasum -a 256 | cut -d' ' -f1)"'"}' \
    >/dev/null

N="$(send_count "{\"audience\":{\"issue\":\"${HITISSUE}\"},\"payload\":{\"title\":\"fixed\"}}")"
[[ "$N" == "1" ]] \
    || { echo "targeting by issue reached ${N} devices, not 1" >&2; exit 1; }

# An issue nobody hit is not everybody.
echo "→ an issue with no identified hits reaches nobody"
N="$(send_count "{\"audience\":{\"issue\":\"${ISSUE_ID}\"},\"payload\":{\"title\":\"t\"}}")"
[[ "$N" == "0" ]] \
    || { echo "an issue with no identified users reached ${N} devices" >&2; exit 1; }

echo "→ an issue id that is not an id is refused"
BADI="$(curl -sS -o /dev/null -w '%{http_code}' -X POST "${BASE}/v1/push/sends" \
    -H "Authorization: Bearer ${API_TOKEN}" -H 'content-type: application/json' \
    -d '{"audience":{"issue":"the login crash"},"payload":{}}')"
[[ "$BADI" == "400" ]] \
    || { echo "a non-id issue answered ${BADI}, not 400" >&2; exit 1; }

echo "→ a preview of an audience that cannot be parsed says so"
BAD="$(curl -sS -o /dev/null -w '%{http_code}' -b "$JAR" -X POST \
    "${BASE}/admin/api/projects/${PROJECT_ID}/push/audience/preview" \
    -H 'content-type: application/json' -d '{"audience":{"trait":"plan","equals":"pro"}}')"
[[ "$BAD" == "400" ]] \
    || { echo "an unparseable audience previewed as ${BAD}, not 400" >&2; exit 1; }

echo "→ signing out clears the traits a send selects on"
curl -fsS -X POST "${BASE}/v1/push/devices" -H "Authorization: Bearer ${TOKEN}" \
    -H 'content-type: application/json' \
    -d '{"kind":"apns","env":"sandbox","nativeToken":"e2e-aud-carol",
         "installId":"e2e-aud-carol","traits":{}}' >/dev/null
N="$(send_count '{"traits":{"plan":"pro","e2e":"aud"},"payload":{"title":"t11"}}')"
[[ "$N" == "1" ]] \
    || { echo "clearing traits left the device selectable: ${N}" >&2; exit 1; }

echo "✓ e2e smoke passed — project ${PROJECT_ID}, issue ${ISSUE_ID}"
