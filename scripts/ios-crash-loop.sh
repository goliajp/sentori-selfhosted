#!/usr/bin/env bash
#
# A real app, a real crash, a real next launch.
#
#   bash scripts/ios-crash-loop.sh
#
# Everything else that tests the crash path tests a piece of it. The
# handler's last act is to restore the default disposition and
# re-raise, so the process it was running in is gone — which means no
# test inside that process can observe what happened next. The unit
# tests suppress the re-raise to get around exactly that, and they
# prove the handler writes a record and that the next launch turns it
# into an event. What they cannot prove is the part in between: that
# the record survives the process dying, and that a launch after a
# crash actually ships it.
#
# So this drives a simulator. An app links the Swift package, calls
# `Sentori.start`, and force-unwraps a nil on command. It dies. It is
# launched again. Then the server is asked whether the crash arrived,
# with the frames and the image identity a dSYM is matched by.
#
# The app is generated into a temp directory, not kept in the
# repository: it exists to be crashed, and a permanent app target is
# a surface that needs a gate of its own.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

command -v xcodegen >/dev/null 2>&1 || { echo "needs xcodegen (brew install xcodegen)" >&2; exit 1; }

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

PORT="$(free_port 8394)"
PGPORT="$(free_port 55437)"
PG_CONTAINER="sentori-crashloop-pg-$$"
BASE="http://127.0.0.1:${PORT}"
APP_DIR="$(mktemp -d)"
LOG="$(mktemp)"
SERVER_PID=""

STARTED_BREW_PG=""
cleanup() {
    [ -n "$SERVER_PID" ] && kill "$SERVER_PID" 2>/dev/null || true
    docker rm -f "$PG_CONTAINER" >/dev/null 2>&1 || true
    # `brew services start` also registers the service to run at login.
    # On a runner that is free; on someone's laptop it is a daemon they
    # did not ask for.
    [ -n "$STARTED_BREW_PG" ] && brew services stop "$STARTED_BREW_PG" >/dev/null 2>&1 || true
    rm -rf "$APP_DIR"
}
trap cleanup EXIT

echo "→ postgres"
# shellcheck source=scripts/lib/local-postgres.sh
. "${ROOT}/scripts/lib/local-postgres.sh"
DBNAME=sentori_crashloop
start_local_postgres

echo "→ server"
cargo build --quiet --manifest-path self-hosted/server/Cargo.toml --bin sentori-server
SENTORI_BIND="127.0.0.1:${PORT}" SENTORI_DATABASE_URL="$DB" \
    SENTORI_OWNER_EMAIL=crash@example.com SENTORI_OWNER_PASSWORD=crash-password-long-enough \
    SENTORI_BASE_URL="$BASE" \
    self-hosted/server/target/debug/sentori-server > "$LOG" 2>&1 &
SERVER_PID=$!
for _ in $(seq 1 60); do curl -fsS "${BASE}/healthz" >/dev/null 2>&1 && break; sleep 1; done
curl -fsS "${BASE}/healthz" >/dev/null || { echo "server never came up:"; tail -20 "$LOG"; exit 1; }

echo "→ project + ingest token"
JAR="$(mktemp)"
curl -fsS -c "$JAR" -X POST "${BASE}/auth/login" -H 'content-type: application/json' \
    -d '{"email":"crash@example.com","password":"crash-password-long-enough"}' >/dev/null
PROJECT="$(curl -fsS -b "$JAR" -X POST "${BASE}/admin/api/projects" \
    -H 'content-type: application/json' -d '{"name":"crash-loop","platform":"ios"}' \
    | python3 -c 'import sys,json;print(json.load(sys.stdin)["id"])')"
TOKEN="$(curl -fsS -b "$JAR" -X POST "${BASE}/admin/api/projects/${PROJECT}/tokens" \
    -H 'content-type: application/json' -d '{"name":"crash","scope":"ingest"}' \
    | python3 -c 'import sys,json;print(json.load(sys.stdin)["token"])')"

echo "→ the app that will die"
mkdir -p "$APP_DIR/CrashHarness"
cat > "$APP_DIR/project.yml" <<YML
name: CrashHarness
options:
  bundleIdPrefix: jp.golia.sentori
  deploymentTarget: { iOS: "18.0" }
settings:
  base:
    SWIFT_VERSION: "5.0"
    SWIFT_STRICT_CONCURRENCY: minimal
packages:
  Sentori:
    path: ${ROOT}/sdk/native/ios
targets:
  CrashHarness:
    type: application
    platform: iOS
    deploymentTarget: "18.0"
    sources: [CrashHarness]
    dependencies:
      - package: Sentori
        product: Sentori
    settings:
      base:
        PRODUCT_BUNDLE_IDENTIFIER: jp.golia.sentori.crashharness
        INFOPLIST_FILE: CrashHarness/Info.plist
        TARGETED_DEVICE_FAMILY: "1"
        CODE_SIGN_IDENTITY: "-"
        CODE_SIGN_STYLE: Automatic
YML

cat > "$APP_DIR/CrashHarness/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>$(PRODUCT_NAME)</string>
  <key>CFBundleIdentifier</key><string>$(PRODUCT_BUNDLE_IDENTIFIER)</string>
  <key>CFBundleExecutable</key><string>$(EXECUTABLE_NAME)</string>
  <key>CFBundlePackageType</key><string>$(PRODUCT_BUNDLE_PACKAGE_TYPE)</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>CFBundleShortVersionString</key><string>1.0</string>
  <key>LSRequiresIPhoneOS</key><true/>
  <key>UILaunchScreen</key><dict/>
</dict></plist>
PLIST

cat > "$APP_DIR/CrashHarness/App.swift" <<'SWIFT'
import Sentori
import SwiftUI
import UIKit

/// A handful of real UIViews, so the wireframe walker has the shapes
/// it knows how to emit.
struct UIKitProbe: UIViewRepresentable {
    func makeUIView(context: Context) -> UIView {
        let host = UIView(frame: CGRect(x: 0, y: 0, width: 300, height: 80))
        host.backgroundColor = .systemGray5
        for (index, colour) in [UIColor.systemRed, .systemGreen, .systemBlue].enumerated() {
            let block = UIView(frame: CGRect(x: 10 + index * 70, y: 10, width: 60, height: 30))
            block.backgroundColor = colour
            host.addSubview(block)
        }
        let label = UILabel(frame: CGRect(x: 10, y: 48, width: 280, height: 24))
        label.text = "a real UILabel"
        host.addSubview(label)
        return host
    }
    func updateUIView(_ view: UIView, context: Context) {}
}

@main
struct CrashHarness: App {
    init() {
        let env = ProcessInfo.processInfo.environment
        Sentori.start(
            SentoriConfig(
                token: env["SENTORI_TOKEN"] ?? "",
                ingestUrl: env["SENTORI_INGEST_URL"] ?? "",
                release: "crashharness@1.0.0+1",
                environment: "test"
            ))
        if env["SENTORI_REPLAY"] == "1" {
            // The replay driver against a real view hierarchy. Its
            // ring is unit-tested against generated frames and its
            // timer is unit-tested for not being on main; what has
            // never run is the capture itself, which reads the view
            // tree and is the reason the timer must not be on main.
            SentoriReplayDriver.start(hz: 4)
            DispatchQueue.main.asyncAfter(deadline: .now() + 4.0) {
                SentoriReplayDriver.stop()
                let ndjson = SentoriReplayDriver.drain()
                let docs = FileManager.default.urls(
                    for: .documentDirectory, in: .userDomainMask
                )[0]
                try? ndjson.write(
                    to: docs.appendingPathComponent("replay-drain.ndjson"),
                    atomically: true, encoding: .utf8
                )
            }
        } else if env["SENTORI_CRASH"] == "1" {
            // A force-unwrapped nil: the single most common way a
            // Swift app dies, and the one an NSException handler
            // never sees. It arrives as EXC_BREAKPOINT / SIGTRAP.
            DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {
                let nothing: Int? = nil
                print("about to die: \(nothing!)")
            }
        } else {
            // The launch after the crash. `start` has already drained
            // and queued whatever the last launch left; push it out
            // rather than waiting for the batch timer.
            DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {
                SentoriTransport.flush()
            }
        }
    }
    // Not an empty screen. A wireframe capture that only ever
    // returned the root window would satisfy "the ring has a
    // keyframe" and be useless, so there has to be a tree to walk.
    var body: some Scene {
        WindowGroup {
            VStack(spacing: 12) {
                Text("crash harness").font(.title)
                // UIKit views on purpose. SwiftUI draws into layers
                // rather than creating a UIView per view, and the
                // wireframe walker emits a node for a UILabel, a
                // UITextView, a UIImageView or anything with a
                // background colour. Which of those two facts
                // explains an empty capture is the question this
                // screen answers.
                UIKitProbe()
                Text("a second label")
            }
        }
    }
}
SWIFT

cd "$APP_DIR"
xcodegen generate >/dev/null
UDID="$(xcrun simctl list devices available -j \
    | python3 -c 'import sys,json;d=json.load(sys.stdin)["devices"];print(next(x["udid"] for rt in d for x in d[rt] if "iPhone" in x["name"]))')"
echo "  simulator $UDID"
xcrun simctl boot "$UDID" >/dev/null 2>&1 || true
xcrun simctl bootstatus "$UDID" -b >/dev/null 2>&1 || true

BUILD_LOG="$(mktemp)"
if ! xcodebuild build -project CrashHarness.xcodeproj -scheme CrashHarness \
        -destination "id=$UDID" -derivedDataPath "$APP_DIR/dd" > "$BUILD_LOG" 2>&1; then
    grep -E "error:" "$BUILD_LOG" | head -20; tail -20 "$BUILD_LOG"; exit 1
fi
APP="$(find "$APP_DIR/dd/Build/Products" -name 'CrashHarness.app' -maxdepth 3 | head -1)"
[ -n "$APP" ] || { echo "no app was built" >&2; exit 1; }

BUNDLE=jp.golia.sentori.crashharness
xcrun simctl uninstall "$UDID" "$BUNDLE" >/dev/null 2>&1 || true
xcrun simctl install "$UDID" "$APP"

echo "→ launch, and die"
SIMCTL_CHILD_SENTORI_TOKEN="$TOKEN" \
SIMCTL_CHILD_SENTORI_INGEST_URL="$BASE" \
SIMCTL_CHILD_SENTORI_CRASH=1 \
    xcrun simctl launch "$UDID" "$BUNDLE" >/dev/null
# The crash fires a second after launch. Polling `launchctl list` for
# the app to disappear looked right and was wrong: it is not listed
# for the first moment either, so the loop fell through immediately
# and the relaunch below killed the instance before it could crash.
#
# So: wait, then require the record. A run where nothing crashed must
# fail here, not fifty seconds later as "no issue arrived", which says
# nothing about which half broke.
sleep 8
CONTAINER="$(xcrun simctl get_app_container "$UDID" "$BUNDLE" data 2>/dev/null || true)"
SIGDIR="$CONTAINER/Documents/sentori/signal"
echo "  signal dir:"
ls -la "$SIGDIR" 2>/dev/null | sed 's/^/    /' || echo "    (none)"
[ -f "$SIGDIR/signal.sentoricrash" ] \
    || { echo "✗ the app did not leave a crash record — it never crashed, or the " \
              "handler never ran" >&2; exit 1; }
[ -f "$SIGDIR/signal.images.json" ] \
    || { echo "✗ no image map beside the record — the next launch cannot attribute " \
              "its addresses" >&2; exit 1; }

echo "→ launch again, and report it"
SIMCTL_CHILD_SENTORI_TOKEN="$TOKEN" \
SIMCTL_CHILD_SENTORI_INGEST_URL="$BASE" \
    xcrun simctl launch "$UDID" "$BUNDLE" >/dev/null

cd "$ROOT"
echo "→ the replay driver against a real view tree"
xcrun simctl terminate "$UDID" "$BUNDLE" >/dev/null 2>&1 || true
rm -f "$CONTAINER/Documents/replay-drain.ndjson"
SIMCTL_CHILD_SENTORI_TOKEN="$TOKEN" \
SIMCTL_CHILD_SENTORI_INGEST_URL="$BASE" \
SIMCTL_CHILD_SENTORI_REPLAY=1 \
    xcrun simctl launch "$UDID" "$BUNDLE" >/dev/null

# Wait for the file, do not assume how long it takes.
#
# This slept eight seconds and then required the drain to exist. That
# is a laptop's eight seconds: a cold CI simulator spends longer than
# that getting the app to its first frame, so the check ran before the
# driver had written anything and reported "produced nothing against a
# real view hierarchy" — which was true, and about the sleep rather
# than about the driver.
DRAIN="$CONTAINER/Documents/replay-drain.ndjson"
WAITED=0
for _ in $(seq 1 60); do
    [ -s "$DRAIN" ] && break
    sleep 1
    WAITED=$((WAITED + 1))
done
if [ ! -s "$DRAIN" ]; then
    echo "✗ the replay driver produced nothing against a real view hierarchy" >&2
    echo "  waited ${WAITED}s for $DRAIN" >&2
    echo "  app running: $(xcrun simctl spawn "$UDID" launchctl list 2>/dev/null | grep -c "$BUNDLE")" >&2
    echo "  Documents holds: $(ls -1 "$CONTAINER/Documents" 2>/dev/null | tr '\n' ' ')" >&2
    exit 1
fi
echo "  drained after ${WAITED}s"
python3 "$ROOT/scripts/lib/check-replay-drain.py" "$DRAIN"

echo "→ did the crash arrive"
FOUND=""
for _ in $(seq 1 40); do
    ISSUES="$(curl -fsS -b "$JAR" "${BASE}/admin/api/issues?projectId=${PROJECT}" || echo '{}')"
    COUNT="$(echo "$ISSUES" | python3 -c 'import sys,json;print(len(json.load(sys.stdin).get("issues") or []))')"
    [ "$COUNT" != "0" ] && { FOUND=1; break; }
    sleep 1
done
[ -n "$FOUND" ] || { echo "✗ no issue arrived after a crash and a relaunch" >&2; tail -20 "$LOG"; exit 1; }

python3 - "$BASE" "$JAR" "$PROJECT" <<'PY'
import json, subprocess, sys
base, jar, project = sys.argv[1], sys.argv[2], sys.argv[3]

def get(url):
    out = subprocess.run(['curl', '-fsS', '-b', jar, url], capture_output=True, text=True).stdout
    return json.loads(out or '{}')

issues = get(f'{base}/admin/api/issues?projectId={project}').get('issues') or []
for issue in issues:
    events = (get(f'{base}/admin/api/issues/{issue["id"]}/events') or {}).get('events') or []
    for ev in events:
        detail = get(f'{base}/admin/api/events/{ev["id"]}')
        payload = detail.get('payload') or {}
        err = payload.get('error') or {}
        stack = err.get('stack') or []
        if not payload.get('nativeCrash'):
            continue
        with_image = [f for f in stack if f.get('imageUuid') and f.get('addr')]
        print(f'  {err.get("type")}: {len(stack)} frames, '
              f'{len(with_image)} with an image identity')
        if not stack:
            sys.exit('✗ the crash arrived with no stack')
        if not with_image:
            sys.exit('✗ no frame carries addr + imageUuid — the server cannot '
                     'symbolicate any of it, and a dSYM would not help')
        print('✓ a real crash, from a dead process, with frames a dSYM can resolve')
        sys.exit(0)
sys.exit('✗ no event marked nativeCrash arrived')
PY
