// The bun that writes our lockfiles and the bun that checks them must
// be the same bun.
//
// 2026-09-06: `build` and `mobile-e2e` went red on master with no
// change to any dependency. CI pinned bun 1.3.13; the lockfiles in
// this repo are written by 1.4.x, and 1.3.13 re-resolves rather than
// reading them — `error: lockfile had changes, but lockfile is
// frozen`, on a tree where `bun install --frozen-lockfile` had just
// passed locally. Preflight could not see it, because preflight ran
// the developer's bun.
//
// A version pinned in four workflow files, two of them as `latest`,
// is not pinned. It lives in `.bun-version` now, every workflow reads
// it with `bun-version-file`, and this checks that the bun running
// preflight is that one — so a green preflight means what CI will
// say, which is the only thing a local gate is for.

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const want = readFileSync(join(root, '.bun-version'), 'utf8').trim();
if (!/^\d+\.\d+\.\d+$/.test(want)) {
  console.error(`✗ .bun-version reads "${want}" — that is not a version.`);
  process.exit(1);
}

const got = execFileSync('bun', ['--version'], { encoding: 'utf8' }).trim();
if (got !== want) {
  console.error(
    `✗ this shell runs bun ${got}; .bun-version says ${want}.\n\n` +
      '    CI installs the version in that file, and the two bunds do not\n' +
      '    agree about lockfiles — 1.3.13 rejects a lockfile 1.4.x wrote.\n' +
      `    Either \`bun upgrade --to ${want}\`, or change .bun-version to\n` +
      `    ${got} and re-run every frozen install so the lockfiles follow.`,
  );
  process.exit(1);
}

// Every workflow reads the file rather than naming a version.
const wf = join(root, '.github/workflows');
const named = [];
for (const f of readdirSync(wf).filter((f) => /\.ya?ml$/.test(f))) {
  const src = readFileSync(join(wf, f), 'utf8');
  for (const [i, l] of src.split('\n').entries()) {
    if (/^\s*bun-version:/.test(l)) named.push(`${f}:${i + 1}  ${l.trim()}`);
  }
}
if (named.length > 0) {
  console.error('✗ a workflow names a bun version instead of reading .bun-version:\n');
  for (const n of named) console.error(`    ${n}`);
  console.error('\n    Use `bun-version-file: .bun-version`. `latest` is not a pin.');
  process.exit(1);
}

// One workspace, one lockfile. `apps/rn-example/bun.lock` sat three
// months stale — Expo 55 for an Expo 57 app — read by no bun the
// developer runs and by the one CI ran.
const stray = [];
function walk(dir) {
  for (const e of readdirSync(dir)) {
    if (e === 'node_modules' || e === '.git' || e === 'target') continue;
    const p = join(dir, e);
    if (statSync(p).isDirectory()) walk(p);
    else if (e === 'bun.lock' || e === 'bun.lockb') stray.push(p.replace(`${root}/`, ''));
  }
}
walk(root);
const members = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8')).workspaces ?? [];
for (const lock of stray) {
  const dir = dirname(lock);
  if (dir === '.') continue;
  if (members.includes(dir)) {
    console.error(
      `✗ ${lock} is inside workspace member \`${dir}\`.\n\n` +
        '    The root lockfile governs a workspace member. A second one is\n' +
        '    read by some bun versions and not others, and drifts unseen.',
    );
    process.exit(1);
  }
}

console.log(`✓ bun ${got} everywhere, and ${stray.length} lockfile(s), none inside a workspace member`);
