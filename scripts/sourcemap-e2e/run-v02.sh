#!/usr/bin/env bash
#
# Round-trip symbolication against the v0.2 server.
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
# Replaces `run.sh`, which drove the v0.1 server via a static dev token
# and `server/migrations`. Neither exists on this stack.

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

echo "[1/6] signing in"
curl -sS -c "$COOKIE" -X POST "$SENTORI_BASE/auth/login" \
  -H 'Content-Type: application/json' \
  -d "{\"email\":\"$SENTORI_OWNER_EMAIL\",\"password\":\"$SENTORI_OWNER_PASSWORD\"}" \
  >/dev/null

echo "[2/6] creating project + ingest token"
PROJECT_ID=$(curl -sS -b "$COOKIE" -X POST "$SENTORI_BASE/admin/api/projects" \
  -H 'Content-Type: application/json' \
  -d "{\"name\":\"sourcemap e2e\",\"slug\":\"$SLUG\"}" | jqp "['id']")

# Two tokens, because the two halves of this test need different
# rights. Uploading a map is a build-time action and needs `admin`;
# sending an event is what a shipped app does and needs `public`. Using
# one token for both would pass while proving neither.
ADMIN_TOKEN=$(curl -sS -b "$COOKIE" -X POST \
  "$SENTORI_BASE/admin/api/projects/$PROJECT_ID/tokens" \
  -H 'Content-Type: application/json' \
  -d '{"kind":"admin","label":"sourcemap-e2e-admin"}' | jqp "['token']")

TOKEN=$(curl -sS -b "$COOKIE" -X POST \
  "$SENTORI_BASE/admin/api/projects/$PROJECT_ID/tokens" \
  -H 'Content-Type: application/json' \
  -d '{"kind":"public","label":"sourcemap-e2e"}' | jqp "['token']")

echo "[3/6] bundling fixture"
# Bun's bundler rather than Metro: what the test needs is a real
# minified bundle and its map, and `bunx metro` on its own has neither
# a Babel preset nor a haste config, so driving it here meant carrying
# a React Native project's worth of setup to produce eight lines of
# JavaScript.
rm -rf "$DIST"
(cd "$HERE" && bun build app.js --outdir "$DIST" --minify --sourcemap=external)

echo "[4/6] uploading the map against release $RELEASE"
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

# The public token is the one inside a shipped app. If it could upload
# a map, anyone with the app could rewrite how a release symbolicates.
echo "      checking a public token is refused"
REFUSED=$(curl -sS -o /dev/null -w '%{http_code}' -X POST \
  "$SENTORI_BASE/v1/releases/$(python3 -c "
import urllib.parse, sys; print(urllib.parse.quote(sys.argv[1], safe=''))
" "$RELEASE")/artifacts" \
  -H "Authorization: Bearer $TOKEN" \
  -F 'kind=sourcemap' -F "file=@$DIST/app.js.map")
if [ "$REFUSED" != "403" ]; then
  echo "FAIL: a public token uploaded an artifact (got $REFUSED, want 403)" >&2
  exit 1
fi
echo "      public upload refused: $REFUSED"

echo "[5/6] throwing inside the minified bundle, sending the stack"
EVENT_JSON=$(node "$HERE/throw-and-format.js" "$DIST/app.js" "$RELEASE")
curl -sS -o /dev/null -w '      ingest=%{http_code}\n' -X POST "$SENTORI_BASE/v1/events" \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  --data-raw "$EVENT_JSON"

echo "[6/6] reading the stored event back"
sleep 1
ISSUE_ID=$(curl -sS -b "$COOKIE" \
  "$SENTORI_BASE/v1/projects/$PROJECT_ID/issues?limit=50" \
  | jqp "[0]['id']")

EVENT_ID=$(curl -sS -b "$COOKIE" \
  "$SENTORI_BASE/v1/projects/$PROJECT_ID/events?issue_id=$ISSUE_ID&limit=1" \
  | jqp "[0]['id']")

FRAME=$(curl -sS -b "$COOKIE" \
  "$SENTORI_BASE/v1/projects/$PROJECT_ID/events/$EVENT_ID" \
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

echo
echo "Source-map e2e: PASSED"
