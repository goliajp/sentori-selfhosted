// English prose sitting in the UI without going through `t()`.
//
// `check-i18n` answers a different question: are the three catalogues
// the same size, and is every key referenced. Both can be perfectly
// true while a screen renders a sentence nobody translated — which is
// how the billing page shipped three of them, visible to every zh and
// ja user, past a gate that reported "3 locales, all referenced".
//
// Deliberately narrow. It looks for string literals that read like
// sentences — three or more words, mostly letters — and ignores the
// places a sentence is legitimately not UI copy. A checker that cried
// wolf on className strings would be turned off within a week, and a
// gate that is off is worse than no gate, because the green still
// looks like an answer.

import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';

// Every directory that renders. It was `pages` + `components` only,
// which left the app shell (App.tsx: the sidebar, the project
// switcher, the footer) and everything under lib/ unread — chrome a
// zh or ja user sees on every single screen.
const ROOTS = ['src'];
/** Not rendered: the catalogues are English by definition. */
const SKIP = /(^|\/)(i18n)(\/|$)/;

/** Code samples, which are compiled by whoever pastes them.
 *
 *  One file, named exactly, rather than a directory — an exemption
 *  wide enough to hide a screen behind is the same as no gate. What
 *  lives here is server code in seven languages, where
 *  `Content-Type: application/json` is a header and not something a
 *  person reads. Flagging it is the crying-wolf this checker's own
 *  design note warns about; translating it would be worse. */
const CODE_SAMPLES = new Set(['src/lib/push-snippets.ts']);

/** Calls whose argument is compared against, never rendered.
 *
 *  `body.includes('BEGIN PUBLIC KEY')` is a PEM band marker. A string
 *  on the right of a comparison is data being matched, and a
 *  translated one would break the match — the opposite of the bug
 *  this checker exists for. Narrow on purpose: these four calls, and
 *  only when the literal is their first argument. */
const CODE_CALLS = /\.(?:includes|startsWith|endsWith|split)\(\s*$/;

/** Props whose value is machinery, not something a person reads. */
const CODE_PROPS =
  /(?:className|class|href|src|to|id|key|type|role|name|htmlFor|rel|target|charSet|viewBox|d|fill|stroke|xmlns|data-[\w-]+|aria-controls|aria-labelledby)\s*=\s*$/;

/** Looks like a sentence rather than an identifier or a class list.
 *
 *  Counts words instead of allow-listing punctuation. The first
 *  version listed the characters a sentence may contain, and anything
 *  outside the list escaped — an em dash or an ellipsis was enough.
 *  That is how `On the project's Tokens page — an st_pk_… string your
 *  SDK sends.` sat in the onboarding guide, untranslated, while the
 *  check reported the file clean. */
function isProse(text) {
  if (!/^[A-Z]/.test(text)) return false;
  if (!text.includes(' ')) return false;
  // Three or more runs of two-plus letters. `flex items-center gap-6`
  // has none longer than a token, and `px-5` has none at all.
  const words = text.match(/[A-Za-z]{2,}/g) ?? [];
  if (words.length < 3) return false;
  // Class lists and paths are mostly separators; prose is mostly
  // letters and spaces.
  const letters = (text.match(/[A-Za-z ]/g) ?? []).length;
  return letters / text.length > 0.75;
}

function walk(dir, out = []) {
  let entries;
  try {
    entries = readdirSync(dir);
  } catch {
    return out; // a root that moved — reported by the zero-file guard
  }
  for (const e of entries) {
    const p = join(dir, e);
    if (SKIP.test(p)) continue;
    if (CODE_SAMPLES.has(p.replace(/^.*?webapp\//, ''))) continue;
    if (statSync(p).isDirectory()) walk(p, out);
    else if (p.endsWith('.tsx') || p.endsWith('.ts')) out.push(p);
  }
  return out;
}

const findings = [];
let scanned = 0;

for (const root of ROOTS) {
  for (const file of walk(root)) {
    scanned += 1;
    const src = readFileSync(file, 'utf8');
    const lines = src.split('\n');

    // Match single- and double-quoted literals. Template literals are
    // skipped: they are nearly always interpolation, and the ones that
    // are not get caught the next time someone reads the screen.
    const re = /(['"])((?:(?!\1)[^\\\n]|\\.)*)\1/g;
    let m;
    while ((m = re.exec(src)) !== null) {
      const text = m[2].replace(/\\(.)/g, '$1').trim();
      if (!isProse(text)) continue;

      const before = src.slice(0, m.index);
      const line = before.split('\n').length;
      const lineText = lines[line - 1] ?? '';

      // Inside t('…') — the whole point is that it is a key.
      if (/\bt\(\s*$/.test(before.slice(-40))) continue;
      // A key being *defined*, not rendered.
      if (/^\s*['"][\w.]+['"]\s*:/.test(lineText)) continue;
      if (CODE_PROPS.test(before.slice(-60))) continue;
      if (CODE_CALLS.test(before.slice(-40))) continue;
      if (/^\s*(import|export)\b/.test(lineText)) continue;
      if (/\/\/|https?:\/\//.test(lineText)) continue;
      // A comment block explaining something.
      if (/^\s*\*/.test(lineText)) continue;

      findings.push({ file, line, text });
    }
  }
}

if (scanned === 0) {
  console.error(
    `✗ no .tsx/.ts under ${ROOTS.join(', ')} — this checker read nothing.\n` +
      '  It scanned two directories for months while the app shell sat\n' +
      '  outside them; a run that reads no files must not print a tick.',
  );
  process.exit(1);
}
if (findings.length === 0) {
  console.log(`✓ ${scanned} files: no hard-coded UI prose`);
  process.exit(0);
}

console.error(`✗ ${findings.length} string(s) render without t():\n`);
for (const f of findings) {
  console.error(`  ${f.file}:${f.line}`);
  console.error(`    ${f.text}\n`);
}
console.error(
  'Each of these shows English to every zh and ja user. Move it into\n' +
    'the three catalogues and read it back through t().',
);
process.exit(1);
