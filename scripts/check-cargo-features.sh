#!/usr/bin/env bash
#
# v2.20: hard-enforce that crates in `server/Cargo.toml` carry every
# feature the server actually needs at runtime, regardless of whether
# `default-features = false` was set. Without this fence, the same
# incident repeats:
#
#   - 2026-06-08 (v1.1.2 hotfix): jsonwebtoken v10 silently panicked
#     because `default-features = false` dropped both `rust_crypto` and
#     `aws_lc_rs`. All JWT sign paths (APNs / FCM / VAPID) panicked in
#     production, queued push_sends piled up.
#   - 2026-06-08 (v1.1.4 hotfix, same session): reqwest dropped HTTP/2
#     because `default-features = false` and the explicit features list
#     was missing `http2`. APNs v2 requires HTTP/2 ALPN — every APNs
#     send failed with a generic `error sending request`.
#
# Both bugs were latent for months and shipped a green build / green tests
# / green clippy. This lint is the bash-level fence that catches the
# pattern at PR time.
#
# Wire-up: `.github/workflows/build.yml` runs this alongside the existing
# `scripts/check-rfc3339.sh` step.
#
# Adding a new crate that must carry features: add an entry to
# REQUIRED_FEATURES below. One feature per line. Comments inside the
# array are not supported — keep extra reasoning in the header above.

set -euo pipefail

CARGO_TOML="${1:-server/Cargo.toml}"

if [[ ! -f "$CARGO_TOML" ]]; then
  echo "error: $CARGO_TOML not found (cwd=$(pwd))" >&2
  exit 2
fi

# Crate→required feature pairs. Enforced on every entry that lives in
# the `[dependencies]` section. Dev-only / build-only deps are
# intentionally skipped.
#
# Both motivating incidents bit at the same Cargo.toml line but for
# different reasons:
#   - reqwest: feature gated behind `default-features = false` — once
#     you opt out, you must explicitly re-enable `http2` for APNs.
#   - jsonwebtoken: v10 splits crypto-provider out of defaults entirely.
#     You must declare `rust_crypto` (or `aws_lc_rs`) regardless of
#     `default-features` — there is no default provider.
# So the check unconditionally requires the listed feature to appear
# in the features list.
#
# Format: "<crate> <feature>" one per line. Parallel-array form (instead
# of `declare -A`) so this script runs under macOS's bash 3.2 too.
REQUIRED_FEATURES_LIST="
jsonwebtoken rust_crypto
reqwest http2
"

# Pull out just the [dependencies] block — `awk` flips on the section
# header and back off at the next `[section]` line.
deps_section=$(awk '
  /^\[dependencies\]/ { in_section=1; next }
  /^\[/               { in_section=0 }
  in_section          { print }
' "$CARGO_TOML")

failed=0
checked=0

while read -r crate required; do
  [[ -z "$crate" ]] && continue
  line=$(echo "$deps_section" | grep -E "^${crate} ?=" || true)
  if [[ -z "$line" ]]; then
    continue   # crate not declared in [dependencies] — nothing to check
  fi
  checked=$((checked + 1))
  if echo "$line" | grep -qE "\"$required\""; then
    echo "ok:   $crate carries required feature '$required'"
  else
    echo "FAIL: $crate missing required feature '$required'"
    echo "      line: $line"
    failed=1
  fi
done <<EOF
$REQUIRED_FEATURES_LIST
EOF

if [[ "$failed" != "0" ]]; then
  echo
  echo "Fix: add the missing feature to the crate's features list in $CARGO_TOML"
  echo "     (jsonwebtoken needs 'rust_crypto' for crypto-provider selection — v1.1.2 incident)"
  echo "     (reqwest needs 'http2' for APNs HTTP/2 ALPN — v1.1.4 incident)"
  exit 1
fi

echo
echo "passed: $checked crate(s) checked, no missing required features"
exit 0
