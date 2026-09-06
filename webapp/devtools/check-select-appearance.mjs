// Every `<select>` in the app has to wear `SELECT_CLASS`.
//
// A native select on macOS paints its own control over whatever
// background it is given, so `bg-surface` did nothing and each dark
// form grew one light-grey box that matched nothing beside it — on
// the credentials form and on the triage filter bar, which is the
// screen people are on all day. `appearance-none` is what stops it,
// and it lives in one place.
//
// Found by looking at a screenshot. So was the fourth one: a
// search-and-replace over the exact class string fixed three of the
// four filters, and the one with `max-w-40` spliced into the middle
// of the same list was left behind — identical in the render to the
// bug it was supposed to fix, and invisible in the diff.
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

const files = walk(SRC);
if (files.length === 0) {
  console.error('✗ no sources found — this checker read nothing and passed');
  process.exit(1);
}

// The definition itself, so a rename cannot leave this checking a
// constant nobody uses any more.
if (!/appearance-none/.test(readFileSync(UI, 'utf8'))) {
  console.error('✗ SELECT_CLASS in components/ui.tsx no longer sets appearance-none');
  process.exit(1);
}

const bad = [];
let seen = 0;
for (const f of files) {
  if (f === UI) continue;
  const src = readFileSync(f, 'utf8');
  for (const m of src.matchAll(/<select\b/g)) {
    seen += 1;
    // To the `>` that closes the opening tag — tracking brace depth,
    // because `onChange={(e) => …}` contains a `>` and stopping at
    // the first one reads four correct elements as four broken ones.
    // (It did. This checker's first version failed the tree it was
    // written to bless.)
    let depth = 0;
    let end = m.index;
    for (let i = m.index; i < src.length; i += 1) {
      const c = src[i];
      if (c === '{') depth += 1;
      else if (c === '}') depth -= 1;
      else if (c === '>' && depth === 0) {
        end = i;
        break;
      }
    }
    if (!/SELECT_CLASS/.test(src.slice(m.index, end))) {
      const line = src.slice(0, m.index).split('\n').length;
      bad.push(`${relative(root, f)}:${line}`);
    }
  }
}

if (bad.length) {
  console.error('✗ a <select> the browser will paint itself:\n');
  for (const b of bad) console.error(`    ${b}`);
  console.error(
    '\nUse `<Select>` from components/ui, or put SELECT_CLASS on the\n' +
      'className. Without `appearance-none` it renders as a light-grey\n' +
      'native control that matches nothing around it.',
  );
  process.exit(1);
}
console.log(`✓ ${seen} raw <select> element(s), all wearing SELECT_CLASS`);
