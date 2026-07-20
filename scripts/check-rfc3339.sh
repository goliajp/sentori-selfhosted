#!/usr/bin/env bash
#
# Phase 29 sub-C: enforce `#[serde(with = "time::serde::rfc3339")]` on
# every `OffsetDateTime` / `Option<OffsetDateTime>` struct field that
# actually gets serialized — i.e. fields inside a struct or enum whose
# preceding `#[derive(...)]` includes `Serialize` or `Deserialize`.
#
# Without the attribute, OffsetDateTime serializes as a 9-element array
# `[2026, 130, 12, ...]` (year, ordinal day, hour, ...) and every
# dashboard `new Date(...)` parses NaN. RFC 3339 is the protocol-locked
# wire shape (docs/protocol.md).
#
# The bash wrapper exists so CI can call `bash scripts/check-rfc3339.sh`
# directly; the actual logic is python3 because tracking brace depth
# + derive-line context across files is awkward in awk.

set -euo pipefail
cd "$(dirname "$0")/.."
exec python3 - <<'PY'
import re, sys
from pathlib import Path

FIELD            = re.compile(r'^(\s+)((?:pub\s+)?\w+):\s*(Option<)?(time::)?OffsetDateTime')
SERDE_OK         = re.compile(r'serde\s*\([^)]*with\s*=')
STRUCT_OR_ENUM   = re.compile(r'^\s*(?:pub\s+)?(?:struct|enum)\s+\w+.*\{')
DERIVE_LINE      = re.compile(r'^\s*#\[derive\(')
ATTR_LINE        = re.compile(r'^\s*#\[')
BLANK_LINE       = re.compile(r'^\s*$')
DOC_COMMENT      = re.compile(r'^\s*///?')

violations = []
for path in sorted(Path('server/src').rglob('*.rs')):
    in_block = False
    depth = 0
    serde_aware = False
    pending_derives = ''  # accumulates derive(...) seen since the last non-attr line
    prev = ''
    for i, line in enumerate(path.read_text().split('\n')):
        if STRUCT_OR_ENUM.match(line):
            in_block = True
            depth = 1
            serde_aware = bool(
                re.search(r'\bSerialize\b|\bDeserialize\b', pending_derives)
            )
            pending_derives = ''
        elif in_block:
            depth += line.count('{') - line.count('}')
            if depth <= 0:
                in_block = False
                serde_aware = False
            elif serde_aware and FIELD.match(line) and not SERDE_OK.search(prev):
                violations.append((path, i + 1, line.rstrip()))
        elif DERIVE_LINE.match(line):
            pending_derives += ' ' + line
        elif ATTR_LINE.match(line) or BLANK_LINE.match(line) or DOC_COMMENT.match(line):
            pass  # keep accumulating; derives can be separated by attrs / docs
        else:
            pending_derives = ''  # anything else cancels
        prev = line

if violations:
    for path, ln, line in violations:
        print(f"{path}:{ln}: missing #[serde(with = ...)]: {line}")
    print()
    print('Fix: add the annotation immediately before each OffsetDateTime field:')
    print('    #[serde(with = "time::serde::rfc3339")]')
    print('    pub created_at: OffsetDateTime,')
    print('For Option<OffsetDateTime>:')
    print('    #[serde(default, with = "time::serde::rfc3339::option")]')
    print('    pub revoked_at: Option<OffsetDateTime>,')
    sys.exit(1)

print('✓ all OffsetDateTime fields in serde-derived structs carry the rfc3339 annotation')
PY
