// A size in the releases panel has to be readable at a glance.
//
// It rendered `Math.round(bytes / 1024) + ' KB'`, so insight-mobile's
// dSYM — 291 MB, an ordinary size for a real iOS one — displayed as
// `297713 KB`. Nobody converts that in their head, and the column
// exists precisely so an operator can tell at a glance what landed.
//
// The rule is not the wording. It is that the number stays small
// enough to read: a size never renders with more than four digits
// before its unit, because that is the point where a unit should have
// changed.
import { formatBytes } from '../src/components/ui.tsx';

const cases = [
  [0, '0 B'],
  [900, '900 B'],
  [1023, '1023 B'],
  [1024, '1.0 KB'],
  [4_194_304, '4.0 MB'],
  [9_154_716, '8.7 MB'],
  // The one that produced this file.
  [304_857_600, '291 MB'],
  [5_497_558_138_880, '5.0 TB'],
  [undefined, '—'],
  [-1, '—'],
  [Number.NaN, '—'],
];

let bad = 0;
for (const [input, want] of cases) {
  const got = formatBytes(input);
  if (got !== want) {
    console.error(`✗ formatBytes(${input}) = "${got}", expected "${want}"`);
    bad += 1;
  }
}

// The property, checked across the whole range rather than at the
// points chosen above — a table of examples only proves the examples.
for (let n = 1; n < 1e15; n *= 1.7) {
  const out = formatBytes(Math.round(n));
  const digits = out.split(' ')[0].replace('.', '').length;
  if (digits > 4) {
    console.error(`✗ formatBytes(${Math.round(n)}) = "${out}" — too many digits to read`);
    bad += 1;
    break;
  }
}

if (bad > 0) process.exit(1);
console.log(`✓ ${cases.length} size renderings, none wider than four digits`);
