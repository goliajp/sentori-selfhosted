// A short hint does not end with a full stop.
//
// A button, a menu item, a form label, a placeholder, a toast, an
// empty state, an error message and a tooltip are labels, not prose.
// Ending one with 。 reads as a translation nobody proofread — and the
// catalogues had 56 of them: "收件箱加载失败。", "リプレイを読み込め
// ませんでした。", every load failure on every page.
//
// Paragraph prose keeps its punctuation. The line between the two is
// drawn where the rule draws it: one sentence and short is a label,
// several sentences is a description.
//
//   node devtools/check-ui-sentence-final.mjs

import { readFileSync } from 'node:fs';
import ts from 'typescript';

const FILES = ['src/i18n/zh.ts', 'src/i18n/ja.ts'];
// Above this a single sentence is a description rather than a label.
// Everything the catalogues held at the time this was written was
// either well under it or clearly a paragraph.
const LABEL_MAX = 34;
const ENDERS = /[。！？]/g;

const bad = [];
let judged = 0;

for (const file of FILES) {
  const text = readFileSync(new URL(`../${file}`, import.meta.url), 'utf8');
  const source = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true);
  function visit(node) {
    if (!ts.isPropertyAssignment(node) || !ts.isStringLiteralLike(node.name) || !ts.isStringLiteralLike(node.initializer)) {
      ts.forEachChild(node, visit);
      return;
    }
    const key = node.name.text, value = node.initializer.text;
    if (!/[一-鿿ぁ-ヿ]/.test(value)) return;
    judged += 1;
    if (!value.endsWith('。')) return;
    if ((value.match(ENDERS) ?? []).length > 1) return; // several sentences: prose
    if (value.length > LABEL_MAX) return; // one long sentence: a description
    const line = source.getLineAndCharacterOfPosition(node.getStart(source)).line + 1;
    bad.push(`${file}:${line}  ${key}  ${value}`);
  }
  visit(source);
}

if (judged < 200) {
  console.error(`✗ judged ${judged} strings. Broken checker, not a broken tree.`);
  process.exit(1);
}

if (bad.length > 0) {
  console.error(`✗ ${bad.length} short UI string(s) ending in a full stop:`);
  for (const b of bad.slice(0, 15)) console.error(`    ${b}`);
  if (bad.length > 15) console.error(`    … and ${bad.length - 15} more`);
  console.error('  A label is not a sentence. Drop the 。');
  process.exit(1);
}

console.log(`✓ ${judged} CJK strings, no short hint ends in a full stop`);
