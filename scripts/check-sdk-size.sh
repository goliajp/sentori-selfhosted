#!/usr/bin/env bash
# Iron-rule dimension 4 (bounded footprint): the SDK bundles a host
# app ships must stay small. Gate on the built lib/ trees of the two
# packages that end up inside an app binary.
#
# Budgets are generous multiples of today's size — this trips on a
# runaway dependency or an accidental asset, not on normal growth.
set -euo pipefail

# KB budgets — built .js only (what actually ships inside the app;
# .d.ts and .map stay in node_modules).
CORE_BUDGET=100
RN_BUDGET=200

fail=0
check() { # name path budget_kb
  local kb
  kb=$(find "$2" -name '*.js' -exec du -ck {} + | tail -1 | cut -f1)
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
  echo "Fix: find what grew (du -sk sdk/*/lib/*), or raise the budget"
  echo "     deliberately in scripts/check-sdk-size.sh with a note."
  exit 1
}
