// Relative markdown links that point at nothing.
//
// `docs/README.md` is the index of the documentation, and four of its
// SDK links were 404s — the files lived in a `docs-site/` that no
// longer exists. A broken link in an index is not a typo; it is the
// index claiming a page exists.
//
//   node scripts/check-doc-links.mjs

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, dirname, normalize } from 'node:path';

const ROOTS = ['docs', 'self-hosted', 'sdk'];

// Run it against a fresh checkout, not a working copy: two links
// repointed at `.claude/CLAUDE.md` passed here and failed in CI,
// because that path is gitignored — it exists on the machine that
// wrote it and nowhere else. A link only its author can follow is
// exactly what this checker is for, and it caught its own author.
const LINK = /\]\(([^)#\s]+)(?:#[^)\s]*)?\)/g;

function walk(dir, out = []) {
  let entries;
  try {
    entries = readdirSync(dir);
  } catch {
    return out;
  }
  for (const e of entries) {
    if (e === 'node_modules' || e === 'lib' || e === 'dist' || e === 'target') continue;
    const p = join(dir, e);
    if (statSync(p).isDirectory()) walk(p, out);
    else if (p.endsWith('.md')) out.push(p);
  }
  return out;
}

const files = ROOTS.flatMap((r) => walk(r));
if (files.length === 0) {
  console.error(`✗ no markdown under ${ROOTS.join(', ')} — this checker read nothing.`);
  process.exit(1);
}

const broken = [];
for (const f of files) {
  for (const m of readFileSync(f, 'utf8').matchAll(LINK)) {
    const href = m[1];
    if (/^([a-z]+:)?\/\//i.test(href) || href.startsWith('mailto:')) continue;
    const target = normalize(join(dirname(f), href));
    try {
      statSync(target);
    } catch {
      broken.push(`${f} → ${href}`);
    }
  }
}

if (broken.length === 0) {
  console.log(`✓ ${files.length} markdown files, every relative link resolves`);
  process.exit(0);
}
for (const b of broken) console.error(`✗ ${b}`);
console.error(
  '\nA link in a document is a claim that the page exists. Fix the path,\n' +
    'or say what happened to it — a silently dropped link reads as if the\n' +
    'page was never there.',
);
process.exit(1);
