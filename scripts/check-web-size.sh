#!/usr/bin/env bash
# Iron-rule dimension 4, for the browser: what a site pays to include
# Sentori, measured the way a browser receives it.
#
#   bash scripts/check-web-size.sh <bundle.js>
#
# Gzipped, because every server serving this file compresses it and an
# uncompressed number would be a bigger, more comfortable one that no
# integrator experiences.
set -euo pipefail

BUNDLE="${1:?usage: check-web-size.sh <bundle.js>}"
BUDGET_KB=25

[ -f "$BUNDLE" ] || { echo "FAIL: no bundle at $BUNDLE — nothing was measured" >&2; exit 1; }

raw=$(wc -c < "$BUNDLE")
gz=$(gzip -9 -c "$BUNDLE" | wc -c)
gz_kb=$(( gz / 1024 ))

printf 'ok:   web bundle %d B raw, %d B gzipped (budget %d KB)\n' "$raw" "$gz" "$BUDGET_KB"
if [ "$gz_kb" -gt "$BUDGET_KB" ]; then
    echo "FAIL: the browser bundle is ${gz_kb} KB gzipped, over the ${BUDGET_KB} KB budget." >&2
    echo "      Find what grew, or raise the budget deliberately with a note." >&2
    exit 1
fi
