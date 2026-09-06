// Every database connection this repo opens must be pinned to UTC.
//
// `now()`, every `timestamptz` we render, the retention deletes'
// `interval` arithmetic and the workers' `extract(epoch FROM …)` are
// all evaluated in the *session's* time zone, which comes from the
// server's config unless the client says otherwise. 185 sites of ours
// depend on it and none of them names a zone, so two deployments of
// the same image can disagree about what a query means and nothing in
// the result says which zone produced it.
//
// `self-hosted/server/src/db.rs` and `self-hosted/cli/src/db.rs` do
// the pinning. This checks the two things that make that true and stay
// true:
//
//   1. Nothing else opens a connection. One new `PgPool::connect` in a
//      new subcommand is all it takes — it compiles, it runs, and it
//      runs in the server's zone.
//   2. The two copies agree. The CLI is a standalone Cargo workspace
//      on purpose (its Docker layer builds without `core/`), so the
//      module cannot be shared and can drift instead.

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const PINNERS = ['self-hosted/server/src/db.rs', 'self-hosted/cli/src/db.rs'];
const ROOTS = ['self-hosted/server/src', 'self-hosted/cli/src'];

function walk(dir, out = []) {
  let entries;
  try { entries = readdirSync(dir); } catch { return out; }
  for (const e of entries) {
    const p = join(dir, e);
    if (p.includes('/target/')) continue;
    if (statSync(p).isDirectory()) walk(p, out);
    else if (p.endsWith('.rs')) out.push(p);
  }
  return out;
}

const findings = [];

// 1 — nobody else connects.
const files = ROOTS.flatMap((r) => walk(join(root, r)));
if (files.length < 20) {
  console.error(
    `✗ found only ${files.length} Rust files under ${ROOTS.join(', ')} — ` +
      'this checker is broken, not the tree. It has read 60 before now.',
  );
  process.exit(1);
}
let scanned = 0;
for (const file of files) {
  const rel = file.replace(`${root}/`, '');
  if (PINNERS.includes(rel)) continue;
  scanned += 1;
  const src = readFileSync(file, 'utf8');
  for (const [i, l] of src.split('\n').entries()) {
    if (/\bPgPool::connect\b|\bPgPoolOptions::new\b/.test(l)) {
      findings.push(
        `${rel}:${i + 1}  opens its own connection\n      ${l.trim()}\n` +
          '      Use `db::connect` / `crate::db::connect` — it pins the zone.',
      );
    }
  }
}

// 2 — the two copies still say the same thing. Compare the code, not
// the prose: the CLI's header documents why the copy exists.
const bodies = PINNERS.map((rel) => {
  let src;
  try {
    src = readFileSync(join(root, rel), 'utf8');
  } catch {
    console.error(`✗ ${rel} is missing — the pin has no implementation.`);
    process.exit(1);
  }
  const code = src
    .split('\n')
    .filter((l) => !/^\s*(\/\/|$)/.test(l))
    .join('\n');
  const start = code.indexOf('const TIME_ZONE');
  const end = code.indexOf('pub async fn connect(');
  if (start < 0 || end < 0 || end < start) {
    console.error(`✗ ${rel} no longer has the shape this checker reads.`);
    process.exit(1);
  }
  return [rel, code.slice(start, end)];
});
if (!/"UTC"/.test(bodies[0][1])) {
  console.error('✗ the pinned zone is not UTC — deliberate? then this checker changes too.');
  process.exit(1);
}
if (bodies[0][1] !== bodies[1][1]) {
  findings.push(
    `${bodies[0][0]} and ${bodies[1][0]} pin differently.\n` +
      '      They are copies on purpose and must stay identical from\n' +
      '      `const TIME_ZONE` to `pub async fn connect(`.',
  );
}

if (findings.length > 0) {
  console.error('✗ the session TimeZone pin has a hole:\n');
  for (const f of findings) console.error(`    ${f}\n`);
  process.exit(1);
}

console.log(
  `✓ ${scanned} files open no connection of their own; both db.rs pin UTC identically`,
);
