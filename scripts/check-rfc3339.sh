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
#
# 2026-07-21: the scan only ever covered `server/src` (the legacy v0.1
# binary), so the v0.2 stack — `self-hosted/server/src` plus the `core/`
# crates whose models it serializes straight to the dashboard — was
# never checked. 62 fields had drifted, which is exactly why every date
# in the v0.2 dashboard rendered as "NaNy ago". All three roots are in
# scope now.
#
# 2026-07-22: a *field* attribute cannot reach a value interpolated into
# `serde_json::json!`, so 26 handlers that build responses with the
# macro were emitting the array shape while passing this check. By then
# the dashboard had moved to Intl.RelativeTimeFormat, which throws on a
# non-finite number rather than printing NaN, so the same drift took
# whole pages down instead of merely looking wrong. Bare OffsetDateTime
# in a json! body is now a violation too; wrap it in
# `crate::wire_time::rfc3339` / `rfc3339_opt`.

set -euo pipefail
cd "$(dirname "$0")/.."
exec python3 - <<'PY'
import re, sys
from pathlib import Path

FIELD            = re.compile(r'^(\s+)((?:pub\s+)?\w+):\s*(Option<)?(time::)?OffsetDateTime')
# A row column read straight into a json! body. `.is_none()` and friends
# consume the value rather than serialise it, so only reads that end the
# expression count.
# `time::OffsetDateTime` and a bare `OffsetDateTime` brought in by a
# `use` are the same type; the check has to spell both, and missing the
# second one hid seventeen call sites across seven handlers.
RAW_JSON_TS      = re.compile(
    r'\.(?:try_)?get::<(?:Option<\s*)?(?:time::)?OffsetDateTime\s*>?\s*, _>\("[^"]+"\)'
)
# `"created_at": model.created_at` — a timestamp-looking field lifted
# out of a struct. Matched by name because the type is not written at
# the call site; the suffix list is the naming convention this codebase
# actually uses for instants.
STRUCT_FIELD_TS  = re.compile(
    r'"\w*(?:_at|_seen|_bucket|timestamp)"\s*:\s*[a-z_][\w.]*\.\w*'
    r'(?:_at|_seen|_bucket|timestamp)\s*,?\s*$'
)
WRAPPED          = re.compile(r'wire_time::rfc3339')
SERDE_OK         = re.compile(r'serde\s*\([^)]*with\s*=')
STRUCT_OR_ENUM   = re.compile(r'^\s*(?:pub\s+)?(?:struct|enum)\s+\w+.*\{')
DERIVE_LINE      = re.compile(r'^\s*#\[derive\(')
ATTR_LINE        = re.compile(r'^\s*#\[')
BLANK_LINE       = re.compile(r'^\s*$')
DOC_COMMENT      = re.compile(r'^\s*///?')

ROOTS = ('server/src', 'self-hosted/server/src', 'core/crates')

violations = []
sources = [
    p
    for root in ROOTS
    for p in sorted(Path(root).rglob('*.rs'))
    if '/target/' not in str(p)
]
# Pass 2 first: bare OffsetDateTime reads in a json! response body.
for path in sources:
    lines = path.read_text().split('\n')
    for i, line in enumerate(lines):
        if WRAPPED.search(line):
            continue
        if not (RAW_JSON_TS.search(line) or STRUCT_FIELD_TS.search(line)):
            continue
        # `created_at: row.get(...)` builds a struct whose field carries
        # the annotation; only `"created_at": row.get(...)` goes out on
        # the wire unmediated. The quotes are the whole difference.
        if not re.search(r'"\w+"\s*:', line):
            continue
        # Consumed rather than serialised (counting unread, comparing) —
        # the shape on the wire is not involved.
        tail = ' '.join(lines[i : i + 4])
        if re.search(r'\.(is_none|is_some|map|unwrap|matches)\b', tail):
            continue
        violations.append((path, i + 1, line.rstrip()))

for path in sources:
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
    print()
    print('Inside serde_json::json!, where there is no field to annotate:')
    print('    "created_at": crate::wire_time::rfc3339(r.get(...)),')
    print('    "sent_at": crate::wire_time::rfc3339_opt(r.try_get(...).ok().flatten()),')
    sys.exit(1)

print('✓ all OffsetDateTime fields in serde-derived structs carry the rfc3339 annotation')
PY
