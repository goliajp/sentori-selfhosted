// Every markdown code fence must close.
//
// An unclosed ```tsx swallowed the whole rest of a README — including
// the `init()` options table, the only prose answer to "what are the
// defaults". On GitHub and on npm it rendered as literal text inside a
// code block. Two other gates read that table happily, because both
// parse the file as text: grep does not care where a fence is, and the
// only thing that noticed was a reader looking at the rendered page.
//
// Cheap to check, and it fails in exactly the way the eye does not.
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';

const ROOT = new URL('..', import.meta.url).pathname;
const ROOTS = ['docs', 'sdk', 'self-hosted', 'webapp', 'core'];
const SKIP = /node_modules|\/target\/|\/lib\/|\/dist\//;

const files = [];
const walk = (d) => {
  let entries;
  try { entries = readdirSync(join(ROOT, d)); } catch { return; }
  for (const e of entries) {
    const rel = `${d}/${e}`;
    if (SKIP.test(rel)) continue;
    if (statSync(join(ROOT, rel)).isDirectory()) walk(rel);
    else if (e.endsWith('.md')) files.push(rel);
  }
};
for (const r of ROOTS) walk(r);
for (const e of readdirSync(ROOT)) if (e.endsWith('.md')) files.push(e);

if (files.length < 20) {
  console.error(
    `✗ found ${files.length} markdown files. This checker is broken, not ` +
      `the tree.`,
  );
  process.exit(1);
}

const bad = [];
for (const rel of files) {
  const lines = readFileSync(join(ROOT, rel), 'utf8').split('\n');
  let open = null;
  lines.forEach((line, i) => {
    // A fence marker starts a line; an indented ``` inside a list is
    // still a fence, so leading spaces are allowed.
    if (!/^\s*```/.test(line)) return;
    if (open === null) open = { line: i + 1, lang: line.trim().slice(3) };
    else open = null;
  });
  if (open) bad.push({ rel, ...open });
}

if (bad.length > 0) {
  console.error(`✗ ${bad.length} markdown file(s) have an unclosed fence:`);
  for (const b of bad)
    console.error(`    ${b.rel}:${b.line}  \`\`\`${b.lang} never closes`);
  console.error(
    '  Everything after it renders as code, including headings and tables.',
  );
  process.exit(1);
}

console.log(`✓ ${files.length} markdown files, every code fence closes`);
