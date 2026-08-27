#!/usr/bin/env node
// The native version in the tree must not already be published.
//
// `swift/1.0.0` was tagged, mirrored and resolvable, and then two
// features landed on the same version number: crash delivery went in
// and `sdk/native/VERSION` still said 1.0.0. Anyone pulling
// `from: "1.0.0"` got an SDK that captured crashes and sent none,
// while the repo said the feature had shipped.
//
// Nothing catches that. The tag is a git object, the version is a
// file, and neither knows about the other until someone compares
// them — which nobody does at the moment it matters, because the
// moment it matters is an ordinary merge.
//
//   node scripts/check-native-version-tag.mjs
//
// Passing means: either this version was never tagged (a release is
// pending, which is the normal state), or it was tagged at exactly
// this commit (the release commit itself).

import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');

function git(...args) {
  try {
    return execFileSync('git', args, { cwd: root, encoding: 'utf8' }).trim();
  } catch {
    return '';
  }
}

const version = readFileSync(join(root, 'sdk/native/VERSION'), 'utf8').trim();
if (!/^\d+\.\d+\.\d+$/.test(version)) {
  console.error(`✗ sdk/native/VERSION reads "${version}", which is not a version.`);
  process.exit(1);
}

const tag = `swift/${version}`;
const tagged = git('rev-parse', '-q', '--verify', `refs/tags/${tag}`);
if (!tagged) {
  console.log(`✓ native ${version} is not tagged yet — a release is pending`);
  process.exit(0);
}

// A tag exists. It is only honest if the sources it points at are the
// ones in the tree; otherwise the published package is missing
// whatever landed since.
// A consumer resolving `from: "x.y.z"` gets the tagged tree, but it
// only ever compiles the library targets — `Tests/` is never built by
// anyone downstream. Requiring a native release for a change to a test
// file spends a Maven Central publish (the org is over its monthly
// quota) on something no consumer can observe.
//
// Anything outside Tests/ still counts, including fixtures the library
// reads at runtime.
const IGNORED = /(^|\/)(Tests|__tests__|androidTest|src\/test)\//;
const changedSince = git('diff', '--name-only', `${tag}..HEAD`, '--', 'sdk/native')
  .split('\n')
  .filter((f) => f.trim() && !IGNORED.test(f))
  .join('\n');
if (!changedSince) {
  console.log(`✓ native ${version} is tagged and sdk/native has not moved since`);
  process.exit(0);
}

console.error(`✗ ${tag} is published, but sdk/native has changed since it was cut:\n`);
for (const f of changedSince.split('\n').slice(0, 20)) console.error(`    ${f}`);
console.error(
  `\nAnyone resolving \`from: "${version}"\` gets the older sources. Bump\n` +
    `sdk/native/VERSION and run \`node scripts/sync-sdk-version.mjs\`, then tag\n` +
    `the release commit.`,
);
process.exit(1);
