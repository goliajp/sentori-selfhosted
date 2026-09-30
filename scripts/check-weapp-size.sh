#!/usr/bin/env bash
# What a mini program pays to include Sentori.
#
#   bash scripts/check-weapp-size.sh
#
# **Raw**, not gzipped. A mini program's limit is on the package as it
# is uploaded: 2 MB for the main package and 20 MB in total across
# subpackages, counted before any compression. Measuring gzip here
# would report a number WeChat does not use.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BUDGET_KB=100
BUNDLE="$(mktemp -t sentori-weapp-XXXX).js"

bunx esbuild "$ROOT/sdk/weapp/lib/index.js" --bundle --format=esm --minify \
    --outfile="$BUNDLE" --log-level=error

raw=$(wc -c < "$BUNDLE")
raw_kb=$(( raw / 1024 ))
rm -f "$BUNDLE"

printf 'ok:   weapp bundle %d B raw (budget %d KB)\n' "$raw" "$BUDGET_KB"
if [ "$raw_kb" -gt "$BUDGET_KB" ]; then
    echo "FAIL: the mini-program bundle is ${raw_kb} KB, over the ${BUDGET_KB} KB budget." >&2
    echo "      The main package limit is 2 MB uncompressed; a monitoring SDK taking" >&2
    echo "      5% of it is already more than it is worth." >&2
    exit 1
fi
