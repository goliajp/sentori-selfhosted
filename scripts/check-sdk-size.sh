#!/usr/bin/env bash
# Iron-rule dimension 4 (bounded footprint): the SDK bundles a host
# app ships must stay small. Gate on the built lib/ trees of the two
# packages that end up inside an app binary.
#
# Budgets are generous multiples of today's size — this trips on a
# runaway dependency or an accidental asset, not on normal growth.
#
# Measured in bytes, not in disk blocks. `du` rounds every file up to
# a 4 KB block, so a third of what this used to report was filesystem
# slack — react-native read 204 KB for 138 KB of code — and the number
# grew with the *file count*: splitting a module in two cost 4 KB of
# budget while shipping nothing extra. Metro bundles bytes. The number
# had crept to exactly the budget that way, leaving no headroom, which
# is the opposite of the "generous multiple" this comment claims.
set -euo pipefail

# KB budgets — built .js only (what actually ships inside the app;
# .d.ts and .map stay in node_modules). Today: core 31, rn 138.
CORE_BUDGET=60
RN_BUDGET=220

fail=0
check() { # name path budget_kb
  local kb count
  # An unbuilt package has no .js at all, and `du` over nothing is an
  # empty string — which compares as neither over nor under a budget.
  # A size gate that passes because it measured nothing is the same
  # kind of green as a test suite that ran zero tests.
  count=$(find "$2" -name '*.js' 2>/dev/null | wc -l | tr -d ' ')
  if [ "$count" -eq 0 ]; then
    echo "FAIL: $1 has no built .js under $2 — run \`bun run build:sdks\` first"
    fail=1
    return
  fi
  kb=$(( $(find "$2" -name '*.js' -exec cat {} + | wc -c) / 1024 ))
  if [ "$kb" -gt "$3" ]; then
    echo "FAIL: $1 lib/ is ${kb} KB (budget ${3} KB)"
    fail=1
  else
    echo "ok:   $1 lib/ ${kb} KB (budget ${3} KB)"
  fi
}

check core sdk/core/lib "$CORE_BUDGET"
check react-native sdk/react-native/lib "$RN_BUDGET"

[ "$fail" -eq 0 ] || {
  echo
  echo "Fix: find what grew (find sdk/*/lib -name '*.js' -size +8k), or raise the budget"
  echo "     deliberately in scripts/check-sdk-size.sh with a note."
  exit 1
}
