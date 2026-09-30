#!/usr/bin/env bash
#
# A real app, a real death by signal, a real next launch — and what the
# system recorded about it.
#
#   bash scripts/android-crash-loop.sh [emulator-5554]
#
# `SentoriExitInfo` reads `getHistoricalProcessExitReasons`, the same
# record Play Console reports on. Everything unit-tested about it is
# pure: the trace parser against a synthetic dump, the dedup against
# synthetic timestamps, the wire shape. None of that touches `collect()`
# — the ActivityManager call, the API-30 guard, and whether what the
# system hands back is the shape the rest of the file assumes.
#
# The iOS equivalent of this script found three defects the unit tests
# were green through, one of which had made every native crash this SDK
# ever delivered unsymbolicatable. This asks Android the same question.
#
# The app is generated into a temp directory: it exists to be killed,
# and a permanent app module would be another surface needing a gate of
# its own. It resolves Sentori from mavenLocal rather than by project
# path, so what dies here is the artifact an integrator gets.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

DEVICE="${1:-emulator-5554}"
adb -s "$DEVICE" get-state >/dev/null 2>&1 || { echo "no device $DEVICE" >&2; exit 1; }
API="$(adb -s "$DEVICE" shell getprop ro.build.version.sdk | tr -d '\r')"
[ "$API" -ge 30 ] || {
    echo "$DEVICE is API $API; ApplicationExitInfo needs 30+. Below that this" >&2
    echo "gate has nothing to say, and saying it quietly is worse." >&2
    exit 1
}
VERSION="$(tr -d '[:space:]' < sdk/native/VERSION)"
echo "→ $DEVICE, API $API, sentori $VERSION"

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

PORT="$(free_port 8396)"
PGPORT="$(free_port 55439)"
PG_CONTAINER="sentori-androidloop-pg-$$"
BASE="http://127.0.0.1:${PORT}"
APP_DIR="$(mktemp -d)"
LOG="$(mktemp)"
SERVER_PID=""
STARTED_BREW_PG=""
BUNDLE=jp.golia.sentori.crashharness

cleanup() {
    [ -n "$SERVER_PID" ] && kill "$SERVER_PID" 2>/dev/null || true
    docker rm -f "$PG_CONTAINER" >/dev/null 2>&1 || true
    [ -n "$STARTED_BREW_PG" ] && brew services stop "$STARTED_BREW_PG" >/dev/null 2>&1 || true
    adb -s "$DEVICE" reverse --remove tcp:"$PORT" >/dev/null 2>&1 || true
    adb -s "$DEVICE" uninstall "$BUNDLE" >/dev/null 2>&1 || true
    rm -rf "$APP_DIR"
}
trap cleanup EXIT

echo "→ postgres"
DBNAME=sentori_androidloop
# shellcheck source=scripts/lib/local-postgres.sh
. "${ROOT}/scripts/lib/local-postgres.sh"
start_local_postgres

echo "→ server"
(cd self-hosted/server && cargo build --quiet)
SENTORI_BIND="127.0.0.1:${PORT}" SENTORI_DATABASE_URL="$DB" \
    SENTORI_OWNER_EMAIL=crash@example.com SENTORI_OWNER_PASSWORD=crash-password-long-enough \
    SENTORI_BASE_URL="$BASE" \
    self-hosted/server/target/debug/sentori-server > "$LOG" 2>&1 &
SERVER_PID=$!
for _ in $(seq 1 60); do curl -fsS "${BASE}/healthz" >/dev/null 2>&1 && break; sleep 1; done
curl -fsS "${BASE}/healthz" >/dev/null || { echo "server never came up:"; tail -20 "$LOG"; exit 1; }

# The emulator's localhost is its own. `adb reverse` puts the host's
# port on the device at the same number, so one ingestUrl is correct on
# both sides and nothing has to know about 10.0.2.2.
adb -s "$DEVICE" reverse tcp:"$PORT" tcp:"$PORT" >/dev/null

echo "→ project + ingest token"
JAR="$(mktemp)"
curl -fsS -c "$JAR" -X POST "${BASE}/auth/login" -H 'content-type: application/json' \
    -d '{"email":"crash@example.com","password":"crash-password-long-enough"}' >/dev/null
PROJECT="$(curl -fsS -b "$JAR" -X POST "${BASE}/admin/api/projects" \
    -H 'content-type: application/json' -d '{"name":"android-crash-loop","platform":"android"}' \
    | python3 -c 'import sys,json;print(json.load(sys.stdin)["id"])')"
TOKEN="$(curl -fsS -b "$JAR" -X POST "${BASE}/admin/api/projects/${PROJECT}/tokens" \
    -H 'content-type: application/json' -d '{"name":"crash","scope":"ingest"}' \
    | python3 -c 'import sys,json;print(json.load(sys.stdin)["token"])')"

echo "→ publish the SDK to mavenLocal"
(cd sdk/native/android && ./gradlew -q publishReleasePublicationToMavenLocal --console=plain)

echo "→ generate the app that will be killed"
PKG_DIR="$APP_DIR/app/src/main/java/jp/golia/sentori/crashharness"
mkdir -p "$PKG_DIR"
cp -R sdk/native/android/gradle "$APP_DIR/gradle"
cp sdk/native/android/gradlew "$APP_DIR/gradlew"
cp sdk/native/android/gradle.properties "$APP_DIR/gradle.properties"

cat > "$APP_DIR/settings.gradle" <<'GRADLE'
pluginManagement {
    repositories { google(); mavenCentral(); gradlePluginPortal() }
}
dependencyResolutionManagement {
    repositories { mavenLocal(); google(); mavenCentral() }
}
rootProject.name = 'crashharness'
include ':app'
GRADLE

cat > "$APP_DIR/app/build.gradle" <<GRADLE
plugins {
    id 'com.android.application' version '8.12.0'
    id 'org.jetbrains.kotlin.android' version '2.0.21'
}
android {
    namespace 'jp.golia.sentori.crashharness'
    compileSdk 35
    defaultConfig {
        applicationId '${BUNDLE}'
        minSdk 24
        targetSdk 35
        versionCode 1
        versionName '1.0'
    }
    compileOptions {
        sourceCompatibility JavaVersion.VERSION_17
        targetCompatibility JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = '17' }
}
dependencies { implementation 'jp.golia.sentori:sentori:${VERSION}' }
GRADLE

cat > "$APP_DIR/app/src/main/AndroidManifest.xml" <<'XML'
<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android">
    <uses-permission android:name="android.permission.INTERNET" />
    <application android:name=".App" android:label="crashharness"
                 android:usesCleartextTraffic="true">
        <activity android:name=".MainActivity" android:exported="true">
            <intent-filter>
                <action android:name="android.intent.action.MAIN" />
                <category android:name="android.intent.category.LAUNCHER" />
            </intent-filter>
        </activity>
    </application>
</manifest>
XML

cat > "$PKG_DIR/App.kt" <<KOTLIN
package jp.golia.sentori.crashharness

import android.app.Activity
import android.app.Application
import android.os.Bundle
import android.widget.TextView
import com.sentori.Sentori
import com.sentori.SentoriConfig
import com.sentori.SentoriTransport

class App : Application() {
    override fun onCreate() {
        super.onCreate()
        Sentori.start(
            SentoriConfig(
                token = "${TOKEN}",
                ingestUrl = "http://127.0.0.1:${PORT}",
                release = "crashharness@1.0.0+1",
                environment = "test",
            ),
            this,
        )
        // start() has already read whatever the system recorded about
        // the last death and queued it. Push it rather than waiting on
        // the batch timer, which is minutes.
        Thread {
            Thread.sleep(2000)
            SentoriTransport.flush()
        }.start()
    }
}

class MainActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(TextView(this).apply { text = "crash harness" })
        if (intent?.getBooleanExtra("die", false) == true) {
            // A signal we send ourselves needs no uid the adb shell may
            // not have — whether it does is a property of the build
            // type, and a gate that silently stops running on user
            // builds is worse than no gate. SIGKILL is not catchable,
            // so what the system records is REASON_SIGNALED and not
            // our own handler dressing it up as a JVM crash.
            android.os.Handler(mainLooper).postDelayed({
                android.os.Process.sendSignal(android.os.Process.myPid(), 9)
            }, 3000)
        }
    }
}
KOTLIN

echo "→ build + install"
BUILD_LOG="$(mktemp)"
if ! (cd "$APP_DIR" && ANDROID_HOME="${ANDROID_HOME:-$HOME/Library/Android/sdk}" \
        ./gradlew -q :app:assembleDebug --console=plain) > "$BUILD_LOG" 2>&1; then
    tail -40 "$BUILD_LOG"; exit 1
fi
APK="$(find "$APP_DIR/app/build/outputs/apk/debug" -name '*.apk' | head -1)"
[ -n "$APK" ] || { echo "no apk was built" >&2; exit 1; }
adb -s "$DEVICE" uninstall "$BUNDLE" >/dev/null 2>&1 || true
adb -s "$DEVICE" install -r "$APK" >/dev/null

# A first launch with nothing to report. Without it the very first
# record read would be from the install itself, and a gate that passes
# on an artefact of its own setup is not looking at the crash path.
echo "→ first launch, nothing to report yet"
adb -s "$DEVICE" shell am start -n "${BUNDLE}/.MainActivity" >/dev/null
sleep 8
[ -n "$(adb -s "$DEVICE" shell pidof "$BUNDLE" | tr -d '\r')" ] \
    || { echo "✗ the app is not running after a launch" >&2; exit 1; }
BEFORE="$(curl -fsS -b "$JAR" "${BASE}/admin/api/issues?projectId=${PROJECT}" \
    | python3 -c 'import sys,json;print(len(json.load(sys.stdin).get("issues") or []))')"
echo "  $BEFORE issues so far"
adb -s "$DEVICE" shell am force-stop "$BUNDLE" >/dev/null
sleep 2

echo "→ launch again, and let it kill itself with a signal"
adb -s "$DEVICE" shell am start -n "${BUNDLE}/.MainActivity" --ez die true >/dev/null
DIED=""
for _ in $(seq 1 30); do
    sleep 1
    [ -z "$(adb -s "$DEVICE" shell pidof "$BUNDLE" | tr -d '\r')" ] && { DIED=1; break; }
done
[ -n "$DIED" ] || { echo "✗ the app survived; there is nothing for the system to record" >&2; exit 1; }
echo "  gone"

# What the system thinks happened, before we ask the SDK. If this says
# something other than a signalled death, the SDK reporting nothing
# would be correct behaviour and the rig is what is wrong.
echo "→ what the system recorded"
adb -s "$DEVICE" shell dumpsys activity exit-info "$BUNDLE" 2>/dev/null \
    | grep -iE "reason|subreason|status" | head -8 || true

echo "→ launch a third time; start() reads the record"
adb -s "$DEVICE" shell am start -n "${BUNDLE}/.MainActivity" >/dev/null

echo "→ did it reach the server"
for _ in $(seq 1 45); do
    NOW="$(curl -fsS -b "$JAR" "${BASE}/admin/api/issues?projectId=${PROJECT}" \
        | python3 -c 'import sys,json;print(len(json.load(sys.stdin).get("issues") or []))')"
    [ "$NOW" -gt "$BEFORE" ] && break
    sleep 1
done
[ "$NOW" -gt "$BEFORE" ] || {
    echo "✗ still $NOW issues after a signalled death and a relaunch" >&2
    adb -s "$DEVICE" logcat -d 2>/dev/null | grep -i sentori | tail -20 >&2 || true
    tail -20 "$LOG" >&2
    exit 1
}
python3 "$ROOT/scripts/lib/check-exit-info.py" "$BASE" "$JAR" "$PROJECT"
