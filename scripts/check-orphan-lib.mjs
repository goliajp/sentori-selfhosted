#!/usr/bin/env node
// A compiled module with no source is still published.
//
//   node scripts/check-orphan-lib.mjs
//
// `tsc` writes into `lib/` and never removes an output whose source is
// gone, and every SDK package lists `lib/` in `files` — so the five
// modules deleted as dead code kept being compiled into the tarball,
// and `sdk/cli/lib/source-bundle.js` was committed that way. An
// integrator could import any of them and get working code for a
// feature this repo believes it removed.
//
// The build now clears `lib/` first. This is the check that says so,
// because "we changed the build script" is not evidence.

import { existsSync, readdirSync, statSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const PACKAGES = ['sdk/core', 'sdk/react-native', 'sdk/expo', 'sdk/cli'];

function walk(dir, out = []) {
  let entries;
  try {
    entries = readdirSync(dir);
  } catch {
    return out;
  }
  for (const e of entries) {
    const p = join(dir, e);
    if (statSync(p).isDirectory()) walk(p, out);
    else if (p.endsWith('.js')) out.push(p);
  }
  return out;
}

const problems = [];
let checked = 0;
let built = 0;

for (const pkg of PACKAGES) {
  const lib = join(root, pkg, 'lib');
  const files = walk(lib);
  // An unbuilt package proves nothing. Saying so beats passing.
  if (files.length === 0) continue;
  built += 1;
  for (const f of files) {
    checked += 1;
    const stem = relative(lib, f).replace(/\.js$/, '');
    const src = join(root, pkg, 'src', stem);
    if (!existsSync(`${src}.ts`) && !existsSync(`${src}.tsx`)) {
      problems.push(`${pkg}/lib/${stem}.js has no ${pkg}/src/${stem}.ts — it ships anyway`);
    }
  }
}

if (built === 0) {
  console.error('✗ no package has a built lib/ — run `bun run build:sdks` first');
  process.exit(1);
}
if (problems.length) {
  for (const p of problems) console.error(`✗ ${p}`);
  console.error('\nRebuild (the build clears lib/ now), and delete any committed leftovers.');
  process.exit(1);
}
console.log(`✓ ${checked} compiled modules across ${built} packages, each with a source`);
