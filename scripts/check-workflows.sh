#!/usr/bin/env bash
# Validate .github/workflows/* with actionlint.
#
# A workflow file that GitHub can't parse is accepted by `git push`
# and then fails as a run with zero jobs and no readable log — which
# is how a literal empty `${{ }}` inside a run-step comment silently
# broke the GHCR image workflow for four consecutive runs on
# 2026-07-20. actionlint catches that class before push.
#
# Soft-skips when actionlint isn't installed so the gate never blocks
# a contributor who hasn't got it; CI installs it explicitly.
set -euo pipefail

if ! command -v actionlint >/dev/null 2>&1; then
  echo "  actionlint not installed — skipping (brew install actionlint)"
  exit 0
fi

actionlint
echo "  ✓ workflows clean"
