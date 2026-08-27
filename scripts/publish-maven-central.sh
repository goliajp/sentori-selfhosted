#!/usr/bin/env bash
# Publish the Android SDK to Maven Central.
#
#   bash scripts/publish-maven-central.sh            # validate only
#   bash scripts/publish-maven-central.sh --publish   # validate, then publish
#
# The Portal takes a **zipped bundle** by POST, not a Maven repository
# by PUT. `build.gradle` used to declare a `central` maven repository
# pointed at the upload endpoint, which could not have worked; nothing
# ever ran it, so nothing ever said so, and the repo carried a
# publishing path that was documented as ready and had never been
# tried. This is that path, written down after being run.
#
# Requires:
#   CENTRAL_USERNAME / CENTRAL_PASSWORD  — a Portal user token
#   SIGNING_KEY / SIGNING_PASSWORD       — an armoured private key
#
# CI reads all four from repository secrets, so a release needs
# nothing from anyone's machine. Running this by hand needs the key,
# which lives in `.secrets/gpg/` (gitignored, never committed):
#
#   export GNUPGHOME="$PWD/.secrets/gpg"
#   export SIGNING_KEY="$(cat .secrets/gpg/private.asc)"
#   export SIGNING_PASSWORD="$(cat .secrets/gpg/passphrase.txt)"
#
# The key that signed 1.2.2 onwards is 22BD3D63FE94A270, and it is
# not in the default keyring — `gpg --export-secret-keys` without
# GNUPGHOME reports no such key and sends you looking for it
# somewhere else. It was generated into its own home because the
# machine's keyboxd had corrupted itself at the time.
#
# The signing key's public half must be on a keyserver Central reads,
# **with its user ID intact**. keys.openpgp.org strips the UID until
# the address is verified by email, and GnuPG will not import a key
# with no UID — so a key that is only there cannot be checked by
# anyone. keyserver.ubuntu.com serves it whole.
#
# Publishing is irreversible: a released version cannot be withdrawn.
# Without `--publish` this stops at VALIDATED, which can be dropped
# from the portal.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

PUBLISH=0
[ "${1:-}" = "--publish" ] && PUBLISH=1

for v in CENTRAL_USERNAME CENTRAL_PASSWORD SIGNING_KEY SIGNING_PASSWORD; do
  if [ -z "${!v:-}" ]; then
    echo "✗ $v is not set — this cannot sign or upload without it." >&2
    exit 1
  fi
done

VERSION="$(cat sdk/native/VERSION)"
COORD="jp.golia.sentori:sentori:${VERSION}"
STAGING="sdk/native/android/build/staging-repo"
WORK="tmp/central-${VERSION}"
BUNDLE="tmp/sentori-${VERSION}-bundle.zip"

echo "→ staging ${COORD} (signed)"
rm -rf "$STAGING"
(cd sdk/native/android && ./gradlew publishReleasePublicationToBuildDirRepository --console=plain -q)

# The staged tree is what the gate reads; check it before shipping it.
node scripts/check-maven-artifact.mjs

DIR="${STAGING}/jp/golia/sentori/sentori/${VERSION}"
[ -d "$DIR" ] || { echo "✗ nothing staged at ${DIR}" >&2; exit 1; }

echo "→ signatures verify against the published public key"
# Against a keyring holding only what a stranger can fetch. Verifying
# with the local secret keyring proves the file was signed here, not
# that anyone else can check it — which is the thing Central does.
VK="$(mktemp -d)"
chmod 700 "$VK"
KEYID="$(GNUPGHOME="$VK" gpg --batch --import-options show-only --import <<<"$SIGNING_KEY" 2>/dev/null \
  | grep -Eo '[0-9A-F]{40}' | head -1)"
[ -n "$KEYID" ] || { echo "✗ could not read a key id out of SIGNING_KEY" >&2; exit 1; }
curl -sS -m 60 -4 "https://keyserver.ubuntu.com/pks/lookup?op=get&search=0x${KEYID}" \
  | GNUPGHOME="$VK" gpg --batch --import >/dev/null 2>&1 || true
if ! GNUPGHOME="$VK" gpg --batch --verify "${DIR}/sentori-${VERSION}.pom.asc" \
     "${DIR}/sentori-${VERSION}.pom" >/dev/null 2>&1; then
  echo "✗ the signature does not verify against the key on keyserver.ubuntu.com." >&2
  echo "  Central checks it the same way, and would refuse the bundle." >&2
  echo "  Publish the public half:" >&2
  echo "    gpg --export --armor ${KEYID} > pub.asc" >&2
  echo "    curl -4 -X POST https://keyserver.ubuntu.com/pks/add --data-urlencode keytext@pub.asc" >&2
  exit 1
fi
echo "      good signature from ${KEYID}"

echo "→ bundle"
rm -rf "$WORK" "$BUNDLE"
mkdir -p "$WORK"
# Artifacts, their signatures and their checksums. Not
# `maven-metadata.xml` (the Portal writes its own), and not checksums
# *of* the signatures.
(cd "$STAGING" && find jp -type f \
  ! -name 'maven-metadata.xml*' \
  ! -name '*.asc.md5' ! -name '*.asc.sha1' \
  ! -name '*.asc.sha256' ! -name '*.asc.sha512' -print0) \
  | while IFS= read -r -d '' f; do
      mkdir -p "${WORK}/$(dirname "$f")"
      cp "${STAGING}/${f}" "${WORK}/${f}"
    done
(cd "$WORK" && zip -qr "${ROOT}/${BUNDLE}" jp)
echo "      $(cd "$WORK" && find jp -type f | wc -l | tr -d ' ') files, $(wc -c < "$BUNDLE") bytes"

AUTH="$(printf '%s:%s' "$CENTRAL_USERNAME" "$CENTRAL_PASSWORD" | base64 | tr -d '\n')"
TYPE="USER_MANAGED"

echo "→ upload"
ID="$(curl -sS -m 600 -X POST \
  -H "Authorization: Bearer ${AUTH}" \
  -F "bundle=@${BUNDLE}" \
  "https://central.sonatype.com/api/v1/publisher/upload?name=${COORD}&publishingType=${TYPE}")"
case "$ID" in
  *-*-*-*-*) ;;
  *) echo "✗ upload did not return a deployment id: ${ID}" >&2; exit 1;;
esac
echo "      ${ID}"

state() {
  curl -sS -m 60 -X POST -H "Authorization: Bearer ${AUTH}" \
    "https://central.sonatype.com/api/v1/publisher/status?id=${ID}"
}
field() { python3 -c "import sys,json;print(json.load(sys.stdin).get('$1'))"; }

echo "→ validation"
for _ in $(seq 1 60); do
  S="$(state)"
  ST="$(printf '%s' "$S" | field deploymentState)"
  case "$ST" in
    VALIDATED|PUBLISHED) break;;
    FAILED)
      echo "✗ ${ST}" >&2
      printf '%s' "$S" | python3 -m json.tool >&2
      exit 1;;
  esac
  sleep 10
done
[ "$ST" = "VALIDATED" ] || [ "$ST" = "PUBLISHED" ] || {
  echo "✗ still ${ST} after ten minutes" >&2; exit 1; }
printf '%s' "$S" | python3 -c "
import sys, json
d = json.load(sys.stdin)
for w in d.get('warnings') or []:
    print(f'      warning: {w}')
" || true
echo "      ${ST}"

if [ "$PUBLISH" -eq 0 ]; then
  echo "✓ ${COORD} validated. It is not published — rerun with --publish,"
  echo "  or drop it at https://central.sonatype.com/publishing/deployments"
  exit 0
fi

echo "→ publish (irreversible)"
curl -sS -m 120 -X POST -H "Authorization: Bearer ${AUTH}" \
  "https://central.sonatype.com/api/v1/publisher/deployment/${ID}" >/dev/null

for _ in $(seq 1 60); do
  ST="$(state | field deploymentState)"
  [ "$ST" = "PUBLISHED" ] && break
  [ "$ST" = "FAILED" ] && { echo "✗ publish failed" >&2; exit 1; }
  sleep 20
done
[ "$ST" = "PUBLISHED" ] || { echo "✗ still ${ST} after twenty minutes" >&2; exit 1; }

# `PUBLISHED` is the portal's word for it. What decides whether anyone
# can depend on this is whether repo1 serves it, so ask repo1.
echo "→ resolvable from repo1"
BASE="https://repo1.maven.org/maven2/jp/golia/sentori/sentori/${VERSION}"
for _ in $(seq 1 40); do
  CODE="$(curl -sS -m 40 -o /dev/null -w '%{http_code}' "${BASE}/sentori-${VERSION}.pom")"
  [ "$CODE" = "200" ] && break
  sleep 30
done
[ "$CODE" = "200" ] || { echo "✗ repo1 still answers ${CODE} for the POM" >&2; exit 1; }
for f in ".aar" "-sources.jar" "-javadoc.jar" ".pom.asc"; do
  CODE="$(curl -sS -m 40 -o /dev/null -w '%{http_code}' "${BASE}/sentori-${VERSION}${f}")"
  [ "$CODE" = "200" ] || { echo "✗ repo1 answers ${CODE} for sentori-${VERSION}${f}" >&2; exit 1; }
  echo "      sentori-${VERSION}${f}"
done

echo "✓ ${COORD} is on Maven Central"
