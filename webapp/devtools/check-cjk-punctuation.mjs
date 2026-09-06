// Chinese UI text uses full-width punctuation.
//
// zh.ts had 44 half-width marks sitting against a Chinese character
// while ja.ts had 5 — and several strings mixed the two inside one
// sentence ("已存但读不了,符号化用不上:index.android.bundle。"), which
// is what makes this a slip rather than a house style. Nothing rendered
// wrong; it just reads as a translation nobody proofread, on the page
// where an operator is already having a bad day.
//
// Only marks touching a CJK character count, so code, URLs, version
// numbers and English inside a Chinese string are left alone — those
// are correctly half-width and always will be.
import { readFileSync } from 'node:fs';

const CJK = '一-鿿';
const PAIRS = { ',': '，', ';': '；', ':': '：', '!': '！', '?': '？' };
const RE = new RegExp(`[${CJK}][,;:!?]|[,;:!?][${CJK}]`, 'g');

const FILE = 'src/i18n/zh.ts';
const text = readFileSync(new URL(`../${FILE}`, import.meta.url), 'utf8');

// If the file stops holding Chinese at all, this checker has nothing to
// judge and must say so rather than report success.
const cjkCount = (text.match(new RegExp(`[${CJK}]`, 'g')) ?? []).length;
if (cjkCount < 100) {
  console.error(
    `✗ ${FILE} holds only ${cjkCount} CJK characters. This checker is ` +
      `broken, not the tree.`,
  );
  process.exit(1);
}

const bad = [];
text.split('\n').forEach((line, i) => {
  for (const hit of line.match(RE) ?? []) {
    bad.push({ hit, line: i + 1, text: line.trim().slice(0, 90) });
  }
});

if (bad.length > 0) {
  console.error(`✗ ${FILE}: ${bad.length} half-width mark(s) against Chinese:`);
  for (const b of bad.slice(0, 12)) {
    const half = b.hit.replace(new RegExp(`[${CJK}]`, 'g'), '');
    console.error(`    ${FILE}:${b.line}  ${half} → ${PAIRS[half] ?? '?'}`);
    console.error(`      ${b.text}`);
  }
  if (bad.length > 12) console.error(`    … and ${bad.length - 12} more`);
  process.exit(1);
}

console.log(`✓ ${FILE}: Chinese punctuation is full-width`);
