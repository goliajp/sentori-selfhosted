#!/usr/bin/env bash
#
# Round-trip symbolication against the v1 server.
#
# Bundles the fixture with Metro, uploads the map against a release,
# sends a stack captured from the *minified* bundle, and asserts the
# stored event points at the original source.
#
# The unit tests in `self-hosted/server/src/symbolicate.rs` pin the
# resolver's arithmetic against a hand-built map. They cannot tell you
# that a real Metro map, uploaded over HTTP and matched to a release by
# name, produces the right answer — that is this script's whole job.
#
# Replaces `run-v02.sh`; the admin surface and token scopes moved
# with the v1 redesign (ingest|api scopes, flat create responses).

set -euo pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
DIST="$HERE/dist"

: "${SENTORI_BASE:=http://localhost:8080}"
: "${SENTORI_OWNER_EMAIL:?required}"
: "${SENTORI_OWNER_PASSWORD:?required}"

RELEASE="sourcemap-e2e@1.0.0+$(date +%s)"
SLUG="sourcemap-e2e-$(date +%s)"
COOKIE="$(mktemp)"
trap 'rm -f "$COOKIE"' EXIT
mkdir -p "$DIST"

jqp() { python3 -c "import sys,json; print(json.load(sys.stdin)$1)"; }

echo "[1/8] signing in"
curl -sS -c "$COOKIE" -X POST "$SENTORI_BASE/auth/login" \
  -H 'Content-Type: application/json' \
  -d "{\"email\":\"$SENTORI_OWNER_EMAIL\",\"password\":\"$SENTORI_OWNER_PASSWORD\"}" \
  >/dev/null

echo "[2/8] creating project + ingest token"
PROJECT_ID=$(curl -sS -b "$COOKIE" -X POST "$SENTORI_BASE/admin/api/projects" \
  -H 'Content-Type: application/json' \
  -d "{\"name\":\"$SLUG\",\"platform\":\"react-native\"}" | jqp "['id']")

# Two tokens, because the two halves of this test need different
# rights. Uploading a map is a build-time action and needs scope
# `api`; sending an event is what a shipped app does and needs
# `ingest`. Using one token for both would pass while proving neither.
ADMIN_TOKEN=$(curl -sS -b "$COOKIE" -X POST \
  "$SENTORI_BASE/admin/api/projects/$PROJECT_ID/tokens" \
  -H 'Content-Type: application/json' \
  -d '{"name":"sourcemap-e2e-api","scope":"api"}' | jqp "['token']")

TOKEN=$(curl -sS -b "$COOKIE" -X POST \
  "$SENTORI_BASE/admin/api/projects/$PROJECT_ID/tokens" \
  -H 'Content-Type: application/json' \
  -d '{"name":"sourcemap-e2e-ingest","scope":"ingest"}' | jqp "['token']")

echo "[3/8] bundling fixture"
# Bun's bundler rather than Metro: what the test needs is a real
# minified bundle and its map, and `bunx metro` on its own has neither
# a Babel preset nor a haste config, so driving it here meant carrying
# a React Native project's worth of setup to produce eight lines of
# JavaScript.
rm -rf "$DIST"
(cd "$HERE" && bun build app.js --outdir "$DIST" --minify --sourcemap=external)

echo "[4/8] uploading the map against release $RELEASE"
# Uploaded with the ingest token, against the release *name* — the path
# `sentori-cli upload sourcemap` takes and the only one a build pipeline
# can take, since CI has no browser session and does not know the
# project's UUID.
#
# This used to drive the admin route instead. That passed for a month
# while the documented CLI posted to `/admin/api/releases/{name}/
# sourcemaps`, which the v0.2 server never had — a green symbolication
# gate over a 404. Test the path the docs hand people.
curl -sS -o /dev/null -w '      upload=%{http_code}\n' -X POST \
  "$SENTORI_BASE/v1/releases/$(python3 -c "
import urllib.parse, sys; print(urllib.parse.quote(sys.argv[1], safe=''))
" "$RELEASE")/artifacts" \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -F 'kind=sourcemap' \
  -F "file=@$DIST/app.js.map"

# The upload creates the release if the deploy marker has not arrived,
# which is the normal order for a build. Announce it too, so the admin
# listing below has the deploy time it renders.
curl -sS -o /dev/null -X POST "$SENTORI_BASE/v1/deploys" \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d "{\"release\":\"$RELEASE\"}"

# Both routes write one table; assert the admin side sees what the
# token side stored, so the two cannot drift apart unnoticed.
RELEASE_ID=$(curl -sS -b "$COOKIE" \
  "$SENTORI_BASE/admin/api/projects/$PROJECT_ID/releases" \
  | python3 -c "
import sys, json
rs = json.load(sys.stdin)['releases']
hit = next(r for r in rs if r['name'] == '$RELEASE')
print(hit['id'])
")

curl -sS -b "$COOKIE" \
  "$SENTORI_BASE/admin/api/projects/$PROJECT_ID/releases/$RELEASE_ID/artifacts" \
  | python3 -c "
import sys, json
arts = json.load(sys.stdin)['artifacts']
if not any(a['kind'] == 'sourcemap' for a in arts):
    sys.exit('FAIL: token upload is not visible on the admin route: %r' % arts)
print('      admin sees %d artifact(s)' % len(arts))
"

# The ingest token is the one inside a shipped app. If it could upload
# a map, anyone with the app could rewrite how a release symbolicates.
echo "      checking an ingest token is refused"
REFUSED=$(curl -sS -o /dev/null -w '%{http_code}' -X POST \
  "$SENTORI_BASE/v1/releases/$(python3 -c "
import urllib.parse, sys; print(urllib.parse.quote(sys.argv[1], safe=''))
" "$RELEASE")/artifacts" \
  -H "Authorization: Bearer $TOKEN" \
  -F 'kind=sourcemap' -F "file=@$DIST/app.js.map")
if [ "$REFUSED" != "403" ]; then
  echo "FAIL: an ingest token uploaded an artifact (got $REFUSED, want 403)" >&2
  exit 1
fi
echo "      ingest upload refused: $REFUSED"

echo "[5/8] throwing inside the minified bundle, sending the stack"
EVENT_JSON=$(bun "$HERE/throw-and-format.js" "$DIST/app.js" "$RELEASE")
curl -sS -o /dev/null -w '      ingest=%{http_code}\n' -X POST "$SENTORI_BASE/v1/events" \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  --data-raw "$EVENT_JSON"

echo "[6/8] reading the stored event back"
sleep 1
ISSUE_ID=$(curl -sS -b "$COOKIE" \
  "$SENTORI_BASE/admin/api/issues?projectId=$PROJECT_ID&limit=50" \
  | jqp "['issues'][0]['id']")

EVENT_ID=$(curl -sS -b "$COOKIE" \
  "$SENTORI_BASE/admin/api/issues/$ISSUE_ID/events" \
  | jqp "['events'][0]['id']")

FRAME=$(curl -sS -b "$COOKIE" \
  "$SENTORI_BASE/admin/api/events/$EVENT_ID" \
  | python3 -c '
import sys, json
frames = json.load(sys.stdin)["payload"]["error"]["stack"]
# The throw site is at the top of the stack.
# A symbolicated frame carries the flag the server sets; matching on
# the filename alone would also match the un-resolved bundle, which is
# also called app.js.
hit = next((f for f in frames if f.get("symbolicated")), None)
if hit is None:
    print("NONE " + json.dumps([f.get("file") for f in frames]))
else:
    print(f'"'"'{hit.get("file")}:{hit.get("line")} fn={hit.get("function")} '"'"'
          f'"'"'minified={hit.get("minifiedFile")}:{hit.get("minifiedLine")}'"'"')
')

echo "      $FRAME"
case "$FRAME" in
  NONE*)
    echo "FAIL: nothing was symbolicated — the map did not match the bundle" >&2
    exit 1
    ;;
esac

# The minified bundle is one line, so a resolved frame must not be.
# Without this the test would pass on a map that resolved everything to
# line 1 — which is what a mismatched map does.
case "$FRAME" in
  *app.js:1\ *)
    echo "FAIL: resolved to line 1, i.e. back to the minified bundle" >&2
    exit 1
    ;;
esac

# The original coordinates have to survive: without them a stale map
# produces confident nonsense that nobody can detect.
case "$FRAME" in
  *minified=None:None*)
    echo "FAIL: frame was rewritten without keeping its minified position" >&2
    exit 1
    ;;
esac

# ── the same map, against a browser-shaped frame ──────────────────
#
# Everything above sends `file: "app.js"`, a bare basename — which is
# what React Native's stacks look like. A browser's do not: they carry
# the full URL the script was served from, with a host, a directory
# and usually a cache-busting query, and nothing had ever sent one of
# those while `docs/getting-started/web.md` tells a reader to upload
# their maps and expect readable stacks.
#
# What this proves, exactly: a URL-shaped `file` is accepted and
# resolves to the same original position the bare name did. It does
# **not** prove that `comparable_path` strips the scheme or the query
# — removing either of those keeps this green, because the release
# holds one map and `candidates_for` tries every map it has, ranked.
# Choosing correctly between two maps with the same basename in
# different directories is a separate case, and one nothing here
# covers yet.
echo "[7/8] a browser-shaped frame against the same map"

# The minified coordinates the previous step resolved from, so this
# frame points at a position the map genuinely covers. Inventing a line
# would test whether some arbitrary number happens to map, which is a
# different and useless question.
MINPOS=$(curl -sS -b "$COOKIE" "$SENTORI_BASE/admin/api/events/$EVENT_ID" \
  | python3 -c '
import sys, json
frames = json.load(sys.stdin)["payload"]["error"]["stack"]
hit = next(f for f in frames if f.get("symbolicated"))
print(hit["minifiedLine"], hit["minifiedColumn"])
')

WEB_EVENT=$(python3 - "$RELEASE" $MINPOS <<'PYEOF'
import json, sys, uuid
release, line, col = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
print(json.dumps({
    "id": str(uuid.uuid4()),
    "kind": "error",
    "occurredAt": "2026-09-30T00:00:00Z",
    "platform": "web",
    "release": release,
    "environment": "test",
    "payload": {
        "error": {
            "type": "BrowserError",
            "message": "thrown from a bundle served over https",
            "stack": [{
                "file": "https://app.example.com/static/app.js?v=8f21c3",
                "function": "checkout",
                "line": line,
                "column": col,
                "inApp": True,
            }],
        }
    },
}))
PYEOF
)
WEB_ID=$(echo "$WEB_EVENT" | jqp "['id']")

curl -sS -o /dev/null -w '      ingest=%{http_code}\n' -X POST "$SENTORI_BASE/v1/events" \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  --data-raw "$WEB_EVENT"
sleep 1

WEB_FRAME=$(curl -sS -b "$COOKIE" "$SENTORI_BASE/admin/api/events/$WEB_ID" \
  | python3 -c '
import sys, json
frames = json.load(sys.stdin)["payload"]["error"]["stack"]
hit = next((f for f in frames if f.get("symbolicated")), None)
print("NONE" if hit is None else str(hit.get("file")) + ":" + str(hit.get("line")))
')
echo "      $WEB_FRAME"
if [ "$WEB_FRAME" = "NONE" ]; then
  echo "FAIL: a browser-shaped frame (scheme, host, query) did not match the map," >&2
  echo "      so every stack from the web SDK stays minified while the docs say" >&2
  echo "      uploading maps makes them readable." >&2
  exit 1
fi

echo "[8/8] two maps named index.js.map, and the right one is picked"
# The assertion lives in Python: it needs a real column out of a real
# map, and building that in bash meant escaped JSON inside a command
# substitution inside a string — which is where two attempts at this
# went, and neither parsed.
TWO="$(mktemp -d)"
(cd "$HERE" && bun build pages/cart/index.js --outdir "$TWO/pages/cart" --minify --sourcemap=external >/dev/null)
(cd "$HERE" && bun build pages/home/index.js --outdir "$TWO/pages/home" --minify --sourcemap=external >/dev/null)
python3 "$HERE/../lib/two-maps-check.py" \
  "$SENTORI_BASE" "$COOKIE" "$ADMIN_TOKEN" "$TOKEN" "$RELEASE" "$TWO"

echo
echo "Source-map e2e: PASSED"
