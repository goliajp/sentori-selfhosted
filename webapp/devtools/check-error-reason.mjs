// An error banner has to say what the server said.
//
// `formatApiError` produces `500: upstream_unavailable — the database
// refused the connection`. Six of the read paths computed it and threw
// it away, rendering only their own sentence: "could not load the
// instruments panel". That sentence cannot separate a session that
// expired (log in again) from a project you cannot see (ask someone)
// from a database that is down (wait) — three different next moves,
// one message.
//
// Nothing found this by reading the source, because every one of those
// pages is correct on its own terms. It took rendering the error state:
// the mock has had a MOCK_FAIL switch, and nothing had ever run it.
//
// The rule: every `<ErrorBanner>` carries the failure as well as the
// sentence — as a `reason` prop, or as the children themselves.
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const SRC = join(root, 'src');
const UI = join(SRC, 'components/ui.tsx');

function walk(dir, out = []) {
  for (const e of readdirSync(dir)) {
    const p = join(dir, e);
    if (statSync(p).isDirectory()) walk(p, out);
    else if (/\.tsx?$/.test(p)) out.push(p);
  }
  return out;
}

// The prop has to still exist, or this checks a spelling nobody reads.
if (!/reason\?: string \| null/.test(readFileSync(UI, 'utf8'))) {
  console.error('✗ ErrorBanner no longer takes a `reason` — this checker is stale');
  process.exit(1);
}

const bad = [];
let seen = 0;
for (const f of walk(SRC)) {
  if (f === UI) continue;
  const src = readFileSync(f, 'utf8');
  for (const m of src.matchAll(/<ErrorBanner\b/g)) {
    seen += 1;
    // To `</ErrorBanner>`, so children count as carrying the reason.
    const close = src.indexOf('</ErrorBanner>', m.index);
    const el = src.slice(m.index, close === -1 ? src.length : close);
    if (!/reason=|\{[^}]*[eE]rror\b/.test(el)) {
      bad.push(`${relative(root, f)}:${src.slice(0, m.index).split('\n').length}`);
    }
  }
}

if (seen === 0) {
  console.error('✗ no ErrorBanner found — this checker read nothing and passed');
  process.exit(1);
}
if (bad.length) {
  console.error('✗ an error banner that does not say what went wrong:\n');
  for (const b of bad) console.error(`    ${b}`);
  console.error(
    '\nPass the failure as `reason={error}`. A sentence alone cannot tell\n' +
      'an expired session from a server that is down, and those are\n' +
      'different things to do next.',
  );
  process.exit(1);
}
console.log(`✓ ${seen} error banner(s), all carrying what the server said`);
