// A platform value reaches the screen through platformLabel, never raw.
//
// `platform` holds wire values: `javascript`, `weapp`, `unknown`, and
// since the column became nullable, nothing at all. The projects page
// rendered it directly, so a card read `weapp` in a monospace pill
// while two other pages showed the same project's platform as a proper
// name, and a project with no platform got an empty pill — a badge
// that says nothing, which is worse than no badge.
//
// The values also grow: v4 added `web`, `weapp` and `unknown` to the
// three the schema was written for. Every site that renders one raw is
// a place where the next addition shows up as a lowercase identifier.
//
//   node devtools/check-platform-label.mjs

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';

const ROOT = new URL('../src/', import.meta.url).pathname;

const files = [];
const walk = (d) => {
  for (const e of readdirSync(join(ROOT, d))) {
    const rel = d ? `${d}/${e}` : e;
    if (statSync(join(ROOT, rel)).isDirectory()) walk(rel);
    else if (/\.tsx$/.test(e)) files.push(rel);
  }
};
walk('');
if (files.length < 10) {
  console.error(`✗ scanned ${files.length} files. Broken checker, not a broken tree.`);
  process.exit(1);
}

// A platform read in JSX *children* position: the `{` opens after a
// `>` or starts the line. A read passed as a prop or interpolated into
// a template is not text a reader sees, and the first version of this
// regex flagged `key={`${r.release}:${r.platform}`}` — a React key,
// which is never rendered.
const RENDER = /(?:^|>)\s*\{\s*[\w.?[\]]*\bplatform\b(?:\s*\?\?\s*[\w.?[\]]*\bplatform\b)*\s*\}/;

const raw = [];
let rendered = 0;
for (const rel of files) {
  if (rel === 'lib/platform-label.ts') continue;
  readFileSync(join(ROOT, rel), 'utf8')
    .split('\n')
    .forEach((line, i) => {
      if (line.includes('platformLabel(')) {
        rendered += 1;
        return;
      }
      if (RENDER.test(line)) raw.push(`${rel}:${i + 1}  ${line.trim().slice(0, 78)}`);
    });
}

if (raw.length > 0) {
  console.error('✗ platform values rendered raw:');
  for (const r of raw) console.error(`    ${r}`);
  console.error(
    '  Wrap them in platformLabel(value, t). It names the runtime in the ' +
      "reader's language and answers '—' for a project that has no platform.",
  );
  process.exit(1);
}

if (rendered === 0) {
  console.error(
    '✗ found no platformLabel call at all. Either it was renamed, or nothing ' +
      'renders a platform any more — either way this checker is judging nothing.',
  );
  process.exit(1);
}

console.log(`✓ ${rendered} platform rendering(s), all through platformLabel`);
