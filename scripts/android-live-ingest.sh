#!/usr/bin/env bash
# The Kotlin SDK against a real Sentori server.
#
# Every other iOS test asserts the shape this SDK *builds*. That is
# worth nothing if the server rejects it, and a mock would agree with
# whatever mistake the SDK is making — which is how a wire format
# quietly diverges. The one thing that cannot agree with a mistake is
# the server.
#
#   bash scripts/android-live-ingest.sh
#
# Brings up Postgres, builds and starts the server, mints a project
# and an ingest token, runs the live Robolectric test against it, and
# — the part that matters — **fails if that test was skipped**. JUnit
# reports an assumption failure as a pass, so a job that only checks
# the exit code would go green for years without having sent a byte.
#
# Robolectric runs on the JVM, so this needs no emulator: the HTTP is
# real, made by the same `HttpURLConnection` code an app would use.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# Pick a port nothing is holding.
#
# These ran on fixed ports with a fixed container name, which is fine
# on a throwaway VM and wrong on the self-hosted runners this repo
# uses: two jobs on one machine fought over 55434, and one lost with
# "address already in use". Worse, they shared a container name, so
# `docker rm -f` at startup would have deleted the other run's
# database — a green job quietly destroying a red one's evidence.
free_port() {
    python3 - "$1" <<'PORTPY'
import socket, sys
preferred = int(sys.argv[1])
for candidate in [preferred] + [0]:
    s = socket.socket()
    try:
        s.bind(("127.0.0.1", candidate))
        print(s.getsockname()[1])
        s.close()
        break
    except OSError:
        s.close()
PORTPY
}

PORT="${SENTORI_LIVE_PORT:-$(free_port 8393)}"
PGPORT="${SENTORI_LIVE_PGPORT:-$(free_port 55434)}"
# Unique per run, so concurrent jobs cannot delete each other.
PG_CONTAINER="sentori-live-pg-android-$$"
BASE="http://127.0.0.1:${PORT}"
FIXTURE="sdk/native/fixtures/live-server.json"
LOG="$(mktemp)"
SERVER_PID=""

STARTED_BREW_PG=""

cleanup() {
    [ -n "$SERVER_PID" ] && kill "$SERVER_PID" 2>/dev/null || true
    docker rm -f "$PG_CONTAINER" >/dev/null 2>&1 || true
    # `brew services start` also registers the service to run at
    # login. On a CI runner that is free; on someone's laptop it is a
    # background daemon they did not ask for, so put it back.
    [ -n "$STARTED_BREW_PG" ] && brew services stop "$STARTED_BREW_PG" >/dev/null 2>&1 || true
    rm -f "$FIXTURE"
}
trap cleanup EXIT

echo "→ postgres"
if command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1; then
    docker rm -f "$PG_CONTAINER" >/dev/null 2>&1 || true
    docker run -d --name "$PG_CONTAINER" \
        -e POSTGRES_USER=postgres -e POSTGRES_PASSWORD=dev -e POSTGRES_DB=sentori \
        -p "${PGPORT}:5432" postgres:18-alpine >/dev/null
    for _ in $(seq 1 60); do
        docker exec "$PG_CONTAINER" pg_isready -U postgres >/dev/null 2>&1 && break
        sleep 1
    done
    DB="postgres://postgres:dev@127.0.0.1:${PGPORT}/sentori"
else
    # macOS runners have no Docker, and no Postgres either — the image
    # ships neither. Homebrew's trusts the local user on 5432, which
    # is enough for a database that lives for one test run.
    #
    # Nothing here is silenced. The first version sent brew's output
    # to /dev/null and failed four seconds in with `exit code 1` and
    # no reason, which is the same defect this script exists to stop
    # other gates from having.
    FORMULA=""
    for candidate in postgresql@18 postgresql@17 postgresql; do
        if brew list --formula "$candidate" >/dev/null 2>&1; then FORMULA="$candidate"; break; fi
    done
    if [ -z "$FORMULA" ]; then
        echo "  installing postgresql@17"
        brew install postgresql@17
        FORMULA=postgresql@17
    fi
    echo "  starting $FORMULA"
    brew services start "$FORMULA"
    STARTED_BREW_PG="$FORMULA"
    # `brew services` returns before the socket is up.
    READY=""
    for _ in $(seq 1 60); do
        if pg_isready -q -h 127.0.0.1 -p 5432; then READY=1; break; fi
        sleep 1
    done
    if [ -z "$READY" ]; then
        echo "postgres never accepted connections; brew services says:" >&2
        brew services list >&2
        exit 1
    fi
    dropdb --if-exists -h 127.0.0.1 sentori_live_android || true
    createdb -h 127.0.0.1 sentori_live_android
    DB="postgres://$(whoami)@127.0.0.1:5432/sentori_live_android"
fi

echo "→ server"
(cd self-hosted/server && cargo build --quiet)
SENTORI_BIND="127.0.0.1:${PORT}" \
SENTORI_DATABASE_URL="$DB" \
SENTORI_OWNER_EMAIL=live@example.com \
SENTORI_OWNER_PASSWORD=live-password-long-enough \
SENTORI_BASE_URL="$BASE" \
    self-hosted/server/target/debug/sentori-server >"$LOG" 2>&1 &
SERVER_PID=$!

for _ in $(seq 1 60); do curl -fsS "${BASE}/healthz" >/dev/null 2>&1 && break; sleep 1; done
curl -fsS "${BASE}/healthz" >/dev/null || { echo "server never came up:"; cat "$LOG"; exit 1; }

echo "→ project + ingest token"
JAR="$(mktemp)"
curl -fsS -c "$JAR" -X POST "${BASE}/auth/login" -H 'content-type: application/json' \
    -d '{"email":"live@example.com","password":"live-password-long-enough"}' >/dev/null
PROJECT="$(curl -fsS -b "$JAR" -X POST "${BASE}/admin/api/projects" \
    -H 'content-type: application/json' -d '{"name":"kotlin-live","platform":"ios"}' \
    | python3 -c 'import sys,json;print(json.load(sys.stdin)["id"])')"
TOKEN="$(curl -fsS -b "$JAR" -X POST "${BASE}/admin/api/projects/${PROJECT}/tokens" \
    -H 'content-type: application/json' -d '{"name":"live","scope":"ingest"}' \
    | python3 -c 'import sys,json;print(json.load(sys.stdin)["token"])')"

python3 - "$BASE" "$TOKEN" "$FIXTURE" <<'PY'
import json, sys
base, token, out = sys.argv[1], sys.argv[2], sys.argv[3]
open(out, 'w').write(json.dumps({"base": base, "token": token}) + "\n")
PY

# The suite runs through the React Native module, which compiles a
# mirror of sdk/native. A stale mirror would report as "the live test
# did not run", which is true but unhelpful.
node scripts/sync-native-core.mjs --check

echo "→ robolectric against it"
GLOG="$(mktemp)"
MODULE=":goliapkg-sentori-react-native"
( cd apps/rn-example/android && ./gradlew "${MODULE}:testDebugUnitTest" \
    --rerun-tasks --console=plain ) >"$GLOG" 2>&1 || {
    grep -E "FAILED|^e: " "$GLOG" | head -20 || true
    echo "── last 30 lines ──"
    tail -30 "$GLOG" || true
    echo "── server log ──"
    tail -40 "$LOG" || true
    echo "FAIL: the Android suite did not pass" >&2
    exit 1
}

# The whole point. JUnit reports an assumption failure as a pass, so
# without this a missing fixture, a renamed test or a Robolectric that
# never started all read as success.
REPORT="sdk/react-native/android/build/test-results/testDebugUnitTest/TEST-com.sentori.SentoriLiveServerTest.xml"
python3 - "$REPORT" <<'PY2'
import sys, xml.etree.ElementTree as ET
try:
    root = ET.parse(sys.argv[1]).getroot()
except Exception as e:
    sys.exit(f"FAIL: no report for the live test ({e}) — it did not run")
if root.get("skipped") not in (None, "0"):
    sys.exit("FAIL: the live test was skipped — it proved nothing")
if int(root.get("tests", 0)) == 0:
    sys.exit("FAIL: the live test report contains no tests")
print(f"      {root.get('tests')} live test(s), {root.get('failures')} failed")
PY2

echo "→ and the events are readable back"
python3 - "$BASE" "$JAR" "$PROJECT" "$LOG" <<'PY3'
import json, subprocess, sys
base, jar, project, log = sys.argv[1:5]

def get(path):
    out = subprocess.run(["curl", "-fsS", "-b", jar, f"{base}{path}"],
                         capture_output=True, text=True, check=True).stdout
    return json.loads(out)

issues = get(f"/admin/api/issues?projectId={project}&limit=20")["issues"]
kinds = sorted(i["kind"] for i in issues)
want = ["assert", "error", "probe", "warn"]
if kinds != want:
    print("── server log ──", file=sys.stderr)
    print(open(log).read()[-3000:], file=sys.stderr)
    sys.exit(f"FAIL: server stored {kinds}, want {want}")

err = next(i for i in issues if i["kind"] == "error")
events = get(f"/admin/api/issues/{err['id']}/events?limit=1")
ev = get(f"/admin/api/events/{events[list(events)[0]][0]['id']}")
p = ev["payload"]

checks = {
    "userKey": bool(ev.get("userKey")),
    "platform=android": ev.get("platform") == "android",
    "error.type": (p.get("error") or {}).get("type", "").endswith("IllegalStateException"),
    "error.stack": bool((p.get("error") or {}).get("stack")),
    "data": (p.get("data") or {}).get("cartId") == "c_1",
    "context": (p.get("context") or {}).get("tenant") == "acme",
    "signals": bool(p.get("signals")),
    "device.os": (p.get("device") or {}).get("os") == "android",
}
bad = [k for k, ok in checks.items() if not ok]
if bad:
    sys.exit(f"FAIL: missing or wrong on the stored event: {bad}\n{json.dumps(ev, indent=2)[:900]}")
print("      " + " · ".join(checks))
PY3

echo "✓ the server accepts what the Kotlin SDK sends"
