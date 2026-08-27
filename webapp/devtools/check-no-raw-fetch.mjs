// The UI does not call `fetch`. It calls the api client.
//
// One page did, and its failure path told the story: a hand-rolled
// `fetch` that threw `new Error(String(resp.status))` never read the
// body, never built an `ApiError`, and so gave `formatApiError` — the
// function whose whole job is to name the status and the message —
// nothing to name. The banner read `Error: 500` while the server had
// answered `upstream_unavailable — the database refused the
// connection`. The gate on `formatApiError` stayed green throughout,
// because it tests the function and this was never the function's
// input.
//
// Everything the client does for free is what the exception was
// missing: reading the error body, the 401 redirect, the `code` and
// `field` a rejected credential comes back with.
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
// The UI layer. `src/lib` is where the client itself lives, and where
// the code that *generates* snippets for a customer's backend lives —
// that code contains the word `fetch` on purpose, inside a string.
const DIRS = [join(root, 'src/pages'), join(root, 'src/components')];

function walk(dir, out = []) {
  for (const e of readdirSync(dir)) {
    const p = join(dir, e);
    if (statSync(p).isDirectory()) walk(p, out);
    else if (/\.tsx?$/.test(p)) out.push(p);
  }
  return out;
}

const files = DIRS.flatMap((d) => walk(d));
if (files.length === 0) {
  console.error('✗ no UI sources found — this checker read nothing and passed');
  process.exit(1);
}

const bad = [];
for (const f of files) {
  const src = readFileSync(f, 'utf8');
  for (const m of src.matchAll(/\bfetch\s*\(/g)) {
    bad.push(`${relative(root, f)}:${src.slice(0, m.index).split('\n').length}`);
  }
}

if (bad.length) {
  console.error('✗ the UI is calling fetch directly:\n');
  for (const b of bad) console.error(`    ${b}`);
  console.error(
    '\nAdd a method to `Api` in src/lib/api.ts and call that. A raw fetch\n' +
      'throws an error with no body, no status class and no code, so the\n' +
      'banner can only say the number.',
  );
  process.exit(1);
}
console.log(`✓ ${files.length} UI files, none calling fetch directly`);
