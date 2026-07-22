#!/usr/bin/env bash
#
# A .rs file that no `mod` declaration reaches is not compiled, not
# linted, and not tested. It reads like working code in review and in
# search, and it does nothing.
#
# `self-hosted/server/src/rate_limit.rs` sat like that from the day it
# was written: eighty lines of per-token rate limiting, three env
# tunables, a kill switch, a module doc explaining the design — and no
# `mod rate_limit;`. Ingest ran unlimited for months while the file
# describing the limit sat next to it.
#
# Nothing else would have caught it. rustc says nothing about files it
# was never pointed at, clippy never sees them, and coverage tools
# report on what ran, not on what was never built.

set -euo pipefail

cd "$(dirname "$0")/.."

python3 - "$@" <<'PY'
import pathlib
import re
import sys

# Every crate root in the workspace. A crate may declare modules in
# main.rs, lib.rs, or both, so all of them are read before deciding
# anything is orphaned — checking only one is how a first attempt at
# this reported fifty false positives against a server that plainly
# works.
ROOTS = [
    "self-hosted/server/src",
    "server/src",
    "cli/src",
    "saas/server/src",
    "migrate-tool/src",
    "wasm/score/src",
]

orphans = []

for rel in ROOTS:
    root = pathlib.Path(rel)
    if not root.is_dir():
        continue

    declared = set()
    for entry in ("main.rs", "lib.rs"):
        f = root / entry
        if f.exists():
            declared |= set(
                re.findall(r"^\s*(?:pub(?:\([^)]*\))? )?mod (\w+);", f.read_text(), re.M)
            )

    on_disk = {p.stem for p in root.glob("*.rs")} - {"main", "lib"}
    on_disk |= {p.name for p in root.iterdir() if p.is_dir() and (p / "mod.rs").exists()}

    for name in sorted(on_disk - declared):
        orphans.append(f"{rel}/{name}")

if not orphans:
    print("✓ every module is reachable from a crate root")
    sys.exit(0)

print("✗ files no `mod` declaration reaches — rustc never compiles these:\n", file=sys.stderr)
for o in orphans:
    print(f"    {o}", file=sys.stderr)
print(
    "\nAdd the `mod` line, or delete the file. Leaving it is the worst of\n"
    "the three: it looks like the feature exists.",
    file=sys.stderr,
)
sys.exit(1)
PY
