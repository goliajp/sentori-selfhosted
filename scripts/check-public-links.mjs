// A shipped document may not send its reader somewhere they cannot go.
//
// `docs/troubleshooting.md` ended "File an issue on GitHub" with a link
// to `goliajp/sentori`, which is private. Every self-hosted reader who
// followed the one instruction we give for getting help got a 404, and
// the page that told them to is in the public mirror.
//
// The rule is about reachability, not about naming: the private
// repository may be mentioned in a URL nobody is asked to open (a
// generated commit link in a CHANGELOG is one), but a document that
// ships to outside readers may not link to it.
//
//   node scripts/check-public-links.mjs

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { join } from 'node:path';

const ROOT = new URL('..', import.meta.url).pathname;

// What the mirror workflow copies out, read from the workflow rather
// than restated here — the two drifting apart is how a file becomes
// public without anyone deciding it should.
const WF = readFileSync(join(ROOT, '.github/workflows/v0.2-oss-mirror.yml'), 'utf8');
const PRIVATE_REPOS = ['goliajp/sentori'];
const PUBLIC_REPO = 'goliajp/sentori-selfhosted';

if (!WF.includes(PUBLIC_REPO)) {
  console.error(
    `✗ the mirror workflow no longer names ${PUBLIC_REPO}. This checker is ` +
      `reading the wrong thing and must not pass.`,
  );
  process.exit(1);
}

// The documents an outside reader is handed: everything under docs/,
// every README that ships with a package, and the top-level README.
const files = ['README.md'];
const walk = (d) => {
  for (const e of readdirSync(join(ROOT, d))) {
    const rel = `${d}/${e}`;
    if (e === 'node_modules' || e === 'lib' || e === 'dist') continue;
    if (statSync(join(ROOT, rel)).isDirectory()) walk(rel);
    else if (e === 'README.md' || (d.startsWith('docs') && e.endsWith('.md'))) files.push(rel);
  }
};
walk('docs');
for (const pkg of readdirSync(join(ROOT, 'sdk'))) {
  const rel = `sdk/${pkg}/README.md`;
  try {
    statSync(join(ROOT, rel));
    files.push(rel);
  } catch {
    // a package without a README is check-published-readme's business
  }
}

// Tracked files only: an untracked scratch file is not shipped.
const tracked = new Set(
  execFileSync('git', ['ls-files'], { cwd: ROOT, encoding: 'utf8' }).split('\n'),
);
const shipped = files.filter((f) => tracked.has(f));
if (shipped.length < 10) {
  console.error(`✗ found ${shipped.length} shipped documents. Broken checker, not a broken tree.`);
  process.exit(1);
}

const bad = [];
for (const rel of shipped) {
  readFileSync(join(ROOT, rel), 'utf8')
    .split('\n')
    .forEach((line, i) => {
      for (const repo of PRIVATE_REPOS) {
        // The private repo, not the public one whose name contains it.
        const re = new RegExp(`github\\.com/${repo}(?![-\\w])`, 'g');
        if (re.test(line)) bad.push(`${rel}:${i + 1}  ${line.trim().slice(0, 90)}`);
      }
    });
}

if (bad.length > 0) {
  console.error(`✗ ${bad.length} link(s) in shipped documents point at a private repository:`);
  for (const b of bad) console.error(`    ${b}`);
  console.error(`  An outside reader gets a 404. Point them at ${PUBLIC_REPO}.`);
  process.exit(1);
}

console.log(`✓ ${shipped.length} shipped documents, none linking somewhere a reader cannot go`);
