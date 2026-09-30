// A version number already on npm may not stand for different code.
//
// v4 moved the transport kernel into `@goliapkg/sentori-core` and left
// the version at 3.0.0 — the number already published, now meaning
// something else. Seventeen exports existed locally and not on npm.
// `sentori-web` was then published depending on `^3.0.0` and importing
// `createTransport`; publish succeeded, the registry served it, and
// `npm install` worked. It failed at the first `import`:
//
//   SyntaxError: The requested module '@goliapkg/sentori-core' does
//   not provide an export named 'createTransport'
//
// Nothing was watching. Every gate here reads the working tree, where
// the export is present; only the registry knows what that version
// number was already promised to mean.
//
// The rule: for a publishable package whose version is already on npm,
// the public exports must be the same. Adding one means bumping.
//
// Needs the network, like the Maven check beside it, and fails rather
// than skips for the same reason. `SENTORI_SKIP_NPM_CHECK=1` opts out
// explicitly for working offline; CI never sets it.
//
//   node scripts/check-sibling-exports.mjs

import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

const ROOT = new URL('..', import.meta.url).pathname;
const SDK = join(ROOT, 'sdk');

if (process.env.SENTORI_SKIP_NPM_CHECK === '1') {
  console.warn('! SENTORI_SKIP_NPM_CHECK=1 — not asking npm what these versions already mean');
  process.exit(0);
}

/** Names a built entry point re-exports. */
function exportsOf(text) {
  return new Set(
    [...text.matchAll(/export \{([^}]*)\}/g)]
      .flatMap((m) => m[1].split(','))
      .map((s) => s.trim().split(/\s+as\s+/).pop())
      .filter(Boolean),
  );
}

const problems = [];
let compared = 0;
let unpublished = 0;

for (const dir of readdirSync(SDK)) {
  const file = join(SDK, dir, 'package.json');
  if (!existsSync(file)) continue;
  const json = JSON.parse(readFileSync(file, 'utf8'));
  if (json.private) continue;
  const entry = join(SDK, dir, 'lib', 'index.js');
  if (!existsSync(entry)) continue; // not built; the size gates own that

  let meta;
  try {
    const res = await fetch(`https://registry.npmjs.org/${json.name.replace('/', '%2f')}`, {
      signal: AbortSignal.timeout(20000),
    });
    if (res.status === 404) {
      unpublished += 1;
      continue;
    }
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    meta = await res.json();
  } catch (e) {
    console.error(`✗ could not ask npm about ${json.name} (${e.message}).`);
    console.error('  A version number already published is a promise about what it');
    console.error('  contains, and only the registry knows what that promise was.');
    console.error('  Set SENTORI_SKIP_NPM_CHECK=1 to skip, offline and knowingly.');
    process.exit(1);
  }

  if (!meta.versions?.[json.version]) {
    unpublished += 1;
    continue; // this version is not out yet; nothing has been promised
  }

  compared += 1;
  const tarball = meta.versions[json.version].dist.tarball;
  let published;
  try {
    const res = await fetch(tarball, { signal: AbortSignal.timeout(30000) });
    const buf = Buffer.from(await res.arrayBuffer());
    const { execFileSync } = await import('node:child_process');
    published = execFileSync('tar', ['-xzO', 'package/lib/index.js'], {
      input: buf,
      maxBuffer: 64 * 1024 * 1024,
      encoding: 'utf8',
    });
  } catch (e) {
    console.error(`✗ could not read the published ${json.name}@${json.version} (${e.message})`);
    process.exit(1);
  }

  const local = exportsOf(readFileSync(entry, 'utf8'));
  const remote = exportsOf(published);
  const added = [...local].filter((n) => !remote.has(n));
  const removed = [...remote].filter((n) => !local.has(n));
  if (added.length > 0 || removed.length > 0) {
    problems.push(
      `${json.name}@${json.version} is on npm with different exports — ` +
        `${added.length} added (${added.slice(0, 6).join(', ')}${added.length > 6 ? ', …' : ''})` +
        `${removed.length > 0 ? `, ${removed.length} removed (${removed.slice(0, 6).join(', ')})` : ''}. ` +
        'Bump the version rather than changing what this one means.',
    );
  }
}

if (compared === 0 && unpublished === 0) {
  console.error('✗ compared nothing. Broken checker, not a broken tree.');
  process.exit(1);
}

if (problems.length > 0) {
  console.error(`✗ ${problems.length} package(s) whose published version means something else:`);
  for (const p of problems) console.error(`    ${p}`);
  process.exit(1);
}

console.log(
  `✓ ${compared} published version(s) mean what this tree says they mean` +
    `${unpublished > 0 ? `; ${unpublished} not published yet` : ''}`,
);
