#!/usr/bin/env node
// A peer range is a claim about what the SDK works with. This asserts
// the claim contains the version we actually build and test against.
//
// It did not. `@goliapkg/sentori-react-native@6.0.0` shipped with
//
//     "expo-modules-core": ">=56.0.0 <57.0.0"
//
// while this repo compiled and tested against 55.0.25 and the current
// Expo SDK ships 57. The declared window held neither — nothing was
// inside it. A new user running `npm install @goliapkg/sentori-expo`
// on current Expo got ERESOLVE and no package, which is the hardest
// possible failure for the one thing this SDK has to be: easy to
// adopt.
//
// Nothing was watching. peerDependencies are strings; no build step
// reads them, and our own example app is wired with `workspace:*`, so
// it never resolved the published range at all.
//
//   node scripts/check-peer-ranges.mjs

import { existsSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');

/** Packages whose published peer ranges we are on the hook for. */
const PACKAGES = ['sdk/react-native', 'sdk/expo', 'sdk/core', 'sdk/cli'];

/** Where a peer might actually be installed, nearest first. The
 *  example app is the baseline: it is the only place in this repo that
 *  pins a real Expo SDK, so it decides what "the version we test
 *  against" means. */
const LOOKUP = (pkgDir) => [
  join(root, pkgDir, 'node_modules'),
  join(root, 'apps/rn-example/node_modules'),
  join(root, 'node_modules'),
];

/** Peers that are ours, or deliberately not installed here. Workspace
 *  siblings are linked by `workspace:*` and never resolve to a
 *  published version; optional peers a host may or may not have are
 *  not part of our baseline. */
const SKIP = new Set([
  '@goliapkg/sentori-react-native',
  '@goliapkg/sentori-core',
  '@react-native-async-storage/async-storage',
  '@react-native-community/netinfo',
  'react-native-reanimated',
  'react-native-gesture-handler',
]);

function installedVersion(pkgDir, name) {
  for (const base of LOOKUP(pkgDir)) {
    const p = join(base, name, 'package.json');
    if (existsSync(p)) return JSON.parse(readFileSync(p, 'utf8')).version;
  }
  return null;
}

/** Compare two dotted versions. Pre-release tags are cut: we never
 *  pin one, and treating `1.0.0-rc.1` as `1.0.0` errs toward letting a
 *  release candidate satisfy a floor rather than failing the build on
 *  a shape this does not model. */
function cmp(a, b) {
  const pa = String(a).split('-')[0].split('.').map(Number);
  const pb = String(b).split('-')[0].split('.').map(Number);
  for (let i = 0; i < 3; i += 1) {
    const d = (pa[i] ?? 0) - (pb[i] ?? 0);
    if (d !== 0) return d < 0 ? -1 : 1;
  }
  return 0;
}

/** Only the range shapes this repo uses: `>=X`, and `>=X <Y`.
 *  Anything else throws rather than returning true — a range this
 *  cannot read is a range it must not bless. That is the whole
 *  difference between a gate and a decoration. */
function satisfies(version, range) {
  const parts = range.trim().split(/\s+/);
  let ok = true;
  for (const part of parts) {
    const m = /^(>=|<=|<|>|=)?\s*v?(\d+(?:\.\d+)*(?:-[\w.]+)?)$/.exec(part);
    if (!m) throw new Error(`unsupported range syntax: "${range}"`);
    const [, op = '=', v] = m;
    const c = cmp(version, v);
    if (op === '>=') ok &&= c >= 0;
    else if (op === '>') ok &&= c > 0;
    else if (op === '<=') ok &&= c <= 0;
    else if (op === '<') ok &&= c < 0;
    else ok &&= c === 0;
  }
  return ok;
}

const problems = [];
let checked = 0;

for (const pkgDir of PACKAGES) {
  const manifest = join(root, pkgDir, 'package.json');
  if (!existsSync(manifest)) {
    problems.push(`${pkgDir}: no package.json — this list is stale`);
    continue;
  }
  const pkg = JSON.parse(readFileSync(manifest, 'utf8'));
  for (const [name, range] of Object.entries(pkg.peerDependencies ?? {})) {
    if (SKIP.has(name)) continue;
    const version = installedVersion(pkgDir, name);
    if (version === null) {
      problems.push(
        `${pkg.name}: declares a peer on "${name}" (${range}) that is installed nowhere in this repo — ` +
          `so the range has never been checked against anything`,
      );
      continue;
    }
    checked += 1;
    let ok;
    try {
      ok = satisfies(version, range);
    } catch (e) {
      problems.push(`${pkg.name}: peer "${name}" — ${e.message}`);
      continue;
    }
    if (!ok) {
      problems.push(
        `${pkg.name}: peer "${name}" declares ${range}, but this repo builds against ${version}`,
      );
    }
  }
}

if (checked === 0) {
  console.error('✗ no peer ranges were checked — this checker read nothing.');
  process.exit(1);
}
if (problems.length === 0) {
  console.log(`✓ ${checked} peer ranges contain the version this repo builds against`);
  process.exit(0);
}
for (const p of problems) console.error(`✗ ${p}`);
console.error(
  '\nA peer range that excludes the version we build against is not a warning\n' +
    'to the user — npm refuses the install outright, and the SDK is simply\n' +
    'unobtainable on the Expo SDK everyone is on.',
);
process.exit(1);
