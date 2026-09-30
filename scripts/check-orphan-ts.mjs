// A TypeScript module nothing imports ships in no bundle and runs
// nowhere. It reads like working code in review and in search, and it
// does nothing.
//
//   node scripts/check-orphan-ts.mjs
//
// `check-orphan-modules.sh` catches the same thing in Rust, where a
// file with no `mod` declaration is never compiled. TypeScript is
// quieter about it: an unreferenced module still typechecks, still
// lints, and still passes every test — it simply is not in the
// product. `sdk/core/src/session.ts` was 115 lines of session tracking,
// exported from the package index, with no caller anywhere; the
// crash-free rate it existed to feed did not exist for two years.
//
// The rule: every module under a watched `src/` must be reachable
// from its package's entry point by imports. Test files and the entry
// points themselves are roots.
//
// Re-exporting a module from `index.ts` does NOT make it reachable —
// that was exactly `session.ts`'s situation. A module that is only
// exported is part of the public API and must say so deliberately, in
// PUBLIC_API below, so the claim is reviewed rather than assumed.

import { readFileSync, existsSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { dirname, join, resolve, relative } from 'node:path';

const PACKAGES = ['sdk/core', 'sdk/react-native', 'sdk/expo', 'sdk/cli'];

// Modules that exist to be imported by someone else's code, not ours.
// Each one is a promise to a consumer; adding a line here says "this
// is API", which is a different claim from "this is used".
const PUBLIC_API = new Set([
  'sdk/core/src/index.ts',
  'sdk/react-native/src/index.ts',
  'sdk/expo/src/index.ts',
  'sdk/cli/src/index.ts',
]);

const problems = [];
let checked = 0;

const tracked = new Set(
  execFileSync('git', ['ls-files'], { encoding: 'utf8' }).split('\n').filter(Boolean),
);

/** Resolve a specifier the way the bundlers here do. */
function resolveSpec(fromFile, spec) {
  if (!spec.startsWith('.')) return null;
  const base = resolve(dirname(fromFile), spec);
  const candidates = [
    base,
    `${base}.ts`,
    `${base}.tsx`,
    join(base, 'index.ts'),
    join(base, 'index.tsx'),
    // The packages write `./foo.js` for `./foo.ts` (NodeNext).
    base.replace(/\.js$/, '.ts'),
    base.replace(/\.js$/, '.tsx'),
  ];
  for (const c of candidates) {
    const rel = relative(process.cwd(), c);
    if (tracked.has(rel)) return rel;
  }
  return null;
}

function importsOf(file) {
  const src = readFileSync(file, 'utf8');
  const specs = [
    ...src.matchAll(/(?:^|\n)\s*(?:import|export)[\s\S]*?from\s+['"]([^'"]+)['"]/g),
    ...src.matchAll(/\brequire\(\s*['"]([^'"]+)['"]\s*\)/g),
    ...src.matchAll(/\bimport\(\s*['"]([^'"]+)['"]\s*\)/g),
  ].map((m) => m[1]);
  return specs.map((s) => resolveSpec(file, s)).filter(Boolean);
}

for (const pkg of PACKAGES) {
  const entry = `${pkg}/src/index.ts`;
  if (!existsSync(entry)) {
    problems.push(`${entry} does not exist — this checker's package list is stale`);
    continue;
  }
  const all = [...tracked].filter(
    (f) => f.startsWith(`${pkg}/src/`) && /\.tsx?$/.test(f) && !f.endsWith('.d.ts'),
  );
  // Tests are roots: they are not shipped, but a module only a test
  // reaches is still dead product code, so they do not count as
  // reachability either. They are simply not required to be reached.
  const isTest = (f) => f.includes('__tests__') || /\.(test|spec)\.tsx?$/.test(f);

  const seen = new Set();
  const queue = [entry];
  while (queue.length > 0) {
    const file = queue.pop();
    if (seen.has(file)) continue;
    seen.add(file);
    for (const dep of importsOf(file)) if (!seen.has(dep)) queue.push(dep);
  }

  for (const file of all) {
    if (isTest(file) || PUBLIC_API.has(file)) continue;
    checked += 1;
    if (!seen.has(file)) {
      problems.push(
        `${file} is not reachable from ${entry} — it is in the repository and in no bundle`,
      );
    }
  }
}

if (checked === 0) {
  console.error('✗ no modules were checked — this checker is looking at nothing');
  process.exit(1);
}

if (problems.length === 0) {
  console.log(`✓ ${checked} TypeScript modules, every one reachable from its package entry`);
  process.exit(0);
}
for (const p of problems) console.error(`✗ ${p}`);
console.error(
  '\nA module nothing imports ships in no bundle and runs nowhere. Delete it, ' +
    'wire it up, or — if it is API for a consumer — add it to PUBLIC_API with that claim in mind.',
);
process.exit(1);
