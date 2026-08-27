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
#
# The crates to check are read from the workspace manifests, not
# listed here. The first version of this script carried a hardcoded
# list of three roots; the v1 redesign deleted two of them and added
# sixteen core crates, and the script went on printing a tick while
# looking at one crate out of seventeen. A checker whose scope is
# written down separately from the thing it checks drifts away from it
# silently, which is the same failure it exists to catch.

set -euo pipefail

cd "$(dirname "$0")/.."

python3 - "$@" <<'PY'
import pathlib
import re
import sys

MANIFESTS = ["core/Cargo.toml", "self-hosted/server/Cargo.toml"]

MOD_DECL = re.compile(r"^\s*(?:#\[[^\]]*\]\s*)*(?:pub(?:\([^)]*\))?\s+)?mod\s+(\w+)\s*;", re.M)
# `#[path = "x.rs"] mod y;` points somewhere else entirely; treat the
# named file as reached so a legitimate redirect is not an orphan.
PATH_ATTR = re.compile(r'#\[\s*path\s*=\s*"([^"]+)"\s*\]\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+(\w+)\s*;')


def crate_roots():
    """(crate_dir, entry_file) for every crate a gate actually builds."""
    out = []
    for man in MANIFESTS:
        p = pathlib.Path(man)
        if not p.is_file():
            continue
        text = p.read_text()
        base = p.parent
        # The manifest's own crate, when it declares one.
        if re.search(r"^\s*\[package\]", text, re.M):
            out.append(base)
        m = re.search(r"^\s*members\s*=\s*\[(.*?)\]", text, re.M | re.S)
        if m:
            for rel in re.findall(r'"([^"]+)"', m.group(1)):
                out.append(base / rel)
    seen, roots = set(), []
    for d in out:
        for entry in ("src/main.rs", "src/lib.rs"):
            f = d / entry
            if f.is_file() and (d, entry) not in seen:
                seen.add((d, entry))
                roots.append((d, f))
    return roots


def reachable_from(entry: pathlib.Path) -> set[pathlib.Path]:
    """Files a crate root reaches, following `mod` through the tree.

    Only top-level files were compared before, so an orphan one
    directory down — exactly where a module tree gets big enough to
    lose track of a file — was invisible.
    """
    seen: set[pathlib.Path] = set()
    stack = [entry]
    while stack:
        f = stack.pop()
        if f in seen or not f.is_file():
            continue
        seen.add(f)
        text = f.read_text()
        here = f.parent if f.name in ("main.rs", "lib.rs", "mod.rs") else f.with_suffix("")
        redirected = set()
        for rel, name in PATH_ATTR.findall(text):
            redirected.add(name)
            stack.append(here / rel)
        for name in MOD_DECL.findall(text):
            if name in redirected:
                continue
            stack.append(here / f"{name}.rs")
            stack.append(here / name / "mod.rs")
    return seen


roots = crate_roots()
if not roots:
    print(
        "✗ no crate roots resolved from " + ", ".join(MANIFESTS) + "\n"
        "  The manifests moved or their [workspace] members list changed shape.\n"
        "  A run that scans nothing must not report success.",
        file=sys.stderr,
    )
    sys.exit(1)

orphans = []
scanned = 0
for crate_dir, entry in roots:
    src = crate_dir / "src"
    reached = reachable_from(entry)
    for f in sorted(src.rglob("*.rs")):
        scanned += 1
        if f not in reached:
            orphans.append(str(f))

if not orphans:
    print(f"✓ {scanned} module files, all reachable from {len(roots)} crate root(s)")
    sys.exit(0)

print("✗ files no `mod` declaration reaches — rustc never compiles these:\n", file=sys.stderr)
for o in sorted(set(orphans)):
    print(f"    {o}", file=sys.stderr)
print(
    "\nAdd the `mod` line, or delete the file. Leaving it is the worst of\n"
    "the three: it looks like the feature exists.",
    file=sys.stderr,
)
sys.exit(1)
PY
