// Every page under docs/ is reachable from docs/README.md.
//
// `check-doc-links` asks whether a link points at a file that exists.
// This asks the other direction: whether a file has a link pointing at
// it. A page nothing links to is a page nobody finds — it is the
// documentation version of a module nothing imports, and it rots the
// same way, because the next person to change the thing it describes
// has no reason to open it.
//
// `docs/replay-encoding-v2.md` and `docs/runbook/cli-auth.md` were
// both judged user-facing and kept, and neither was reachable from the
// index by any path.
//
//   node scripts/check-doc-reachable.mjs

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { dirname, join, normalize, relative } from 'node:path';

const ROOT = new URL('../docs/', import.meta.url).pathname;
const INDEX = 'README.md';

const pages = [];
const walk = (d) => {
  for (const e of readdirSync(join(ROOT, d))) {
    const rel = d ? `${d}/${e}` : e;
    if (statSync(join(ROOT, rel)).isDirectory()) walk(rel);
    else if (e.endsWith('.md')) pages.push(rel);
  }
};
walk('');

if (pages.length < 5) {
  console.error(`✗ found ${pages.length} pages under docs/. Broken checker, not a broken tree.`);
  process.exit(1);
}

// Breadth-first from the index, so a page linked only from another
// unreachable page is still unreachable — which is the case this
// exists for.
const seen = new Set([INDEX]);
const queue = [INDEX];
while (queue.length > 0) {
  const page = queue.shift();
  let text;
  try {
    text = readFileSync(join(ROOT, page), 'utf8');
  } catch {
    continue;
  }
  for (const m of text.matchAll(/\]\(([^)#\s]+)(?:#[^)\s]*)?\)/g)) {
    const target = m[1];
    if (/^[a-z]+:/.test(target) || !target.endsWith('.md')) continue;
    const resolved = normalize(join(dirname(page), target));
    // A link out of docs/ (../sdk/web/README.md and the like) is a
    // real link, but not a docs page this checker owns.
    if (relative('', resolved).startsWith('..')) continue;
    if (seen.has(resolved)) continue;
    seen.add(resolved);
    queue.push(resolved);
  }
}

const orphans = pages.filter((p) => !seen.has(p)).sort();
if (orphans.length > 0) {
  console.error(`✗ ${orphans.length} page(s) under docs/ that no reader can reach from ${INDEX}:`);
  for (const o of orphans) console.error(`    docs/${o}`);
  console.error(
    '  Link each from the index or from a page that is itself reachable. ' +
      'A page with no way in is one nobody maintains.',
  );
  process.exit(1);
}

console.log(`✓ ${pages.length} docs pages, all reachable from ${INDEX}`);
