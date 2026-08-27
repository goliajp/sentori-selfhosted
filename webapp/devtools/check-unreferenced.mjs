// A module nothing imports. The TypeScript twin of
// `scripts/check-orphan-modules.sh`.
//
// `tsc` type-checks every file under `src`, eslint lints every file
// under `src`, and Vite ships only what `main.tsx` reaches. So a file
// left behind by a redesign passes both gates for as long as it stays
// syntactically valid, and reads in review and in search exactly like
// code that runs.
//
// `src/lib/useShortcuts.ts` was that: a Linear-style two-key
// navigation map to `/main`, `/members`, `/alerts`, `/saved-views`,
// `/audit`, `/health`, `/saas` — none of them routes in this app
// since the v1 redesign — imported by nobody, green in every check.
//
// Exempting something is fine; doing it silently is not. Every entry
// in ALLOWED carries the reason it is allowed to sit there, so the
// list reads as a decision rather than as an accident.

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, normalize, dirname } from 'node:path';

const ENTRY = 'src/main.tsx';
const ROOT = 'src';

/** Paths that are legitimately unreachable from the entry, and why.
 *
 *  Empty, and that is the target state. `src/legal/documents.ts` sat
 *  here for a day — terms, privacy and 特定商取引法 copy for a hosted
 *  offering with no signup and no billing, rendered by pages that
 *  went with the SaaS surface. Kept "pending a decision", which is
 *  what an exemption always says. The decision was to delete it; the
 *  history has the text if a hosted offering ever returns. */
const ALLOWED = new Map([]);

/** Ambient declarations are pulled in by tsconfig, not by an import. */
const AMBIENT = /\.d\.ts$/;

const IMPORT = /(?:from|import)\s*\(?\s*['"](\.[^'"]+)['"]/g;

function walk(dir, out = []) {
  for (const e of readdirSync(dir)) {
    const p = join(dir, e);
    if (statSync(p).isDirectory()) walk(p, out);
    else if (p.endsWith('.ts') || p.endsWith('.tsx')) out.push(normalize(p));
  }
  return out;
}

/** Resolve a relative specifier the way the bundler does — including
 *  the `./foo.js` form TypeScript's NodeNext emits for a `.ts` file. */
function resolve(fromFile, spec) {
  const base = normalize(join(dirname(fromFile), spec));
  const bare = base.endsWith('.js') ? base.slice(0, -3) : base;
  const candidates = [
    base,
    `${bare}.ts`,
    `${bare}.tsx`,
    join(base, 'index.ts'),
    join(base, 'index.tsx'),
  ];
  for (const c of candidates) {
    try {
      if (statSync(c).isFile()) return normalize(c);
    } catch {
      /* not this one */
    }
  }
  return null;
}

const all = walk(ROOT);
if (all.length === 0) {
  console.error(
    `✗ no modules found under ${ROOT}/ — this checker scanned nothing.\n` +
      '  A run that reads no files must not report success.',
  );
  process.exit(1);
}

const reached = new Set();
const stack = [normalize(ENTRY)];
while (stack.length > 0) {
  const f = stack.pop();
  if (reached.has(f)) continue;
  reached.add(f);
  let src;
  try {
    src = readFileSync(f, 'utf8');
  } catch {
    continue;
  }
  for (const m of src.matchAll(IMPORT)) {
    const r = resolve(f, m[1]);
    if (r) stack.push(r);
  }
}
if (reached.size < 2) {
  console.error(
    `✗ ${ENTRY} reached ${reached.size} file(s) — the entry point moved or ` +
      'the import pattern no longer matches. Everything would look ' +
      'unreferenced, so this is a broken checker, not a broken tree.',
  );
  process.exit(1);
}

const orphans = all.filter(
  (f) => !reached.has(f) && !AMBIENT.test(f) && !ALLOWED.has(f),
);

// An allow-list entry for a file that no longer exists is stale, and a
// stale exemption is how the next real orphan gets waved through.
const stale = [...ALLOWED.keys()].filter((f) => !all.includes(normalize(f)));
if (stale.length > 0) {
  console.error('✗ ALLOWED names files that are not there any more:\n');
  for (const f of stale) console.error(`    ${f}`);
  console.error('\nDelete the entry — an exemption nobody can check is not one.');
  process.exit(1);
}

if (orphans.length === 0) {
  const n = ALLOWED.size;
  console.log(
    `✓ ${reached.size} modules reachable from ${ENTRY}` +
      (n > 0 ? `, ${n} allowed unreferenced` : ''),
  );
  process.exit(0);
}

console.error(`✗ ${orphans.length} module(s) nothing imports:\n`);
for (const f of orphans) console.error(`    ${f}`);
console.error(
  '\ntsc checks these and eslint lints them, so they look alive in every\n' +
    'gate while shipping to nobody. Delete them, wire them up, or add\n' +
    'them to ALLOWED in this file with the reason.',
);
process.exit(1);
