// A crash-free rate never reads as perfect when something crashed.
//
//   node devtools/check-crash-free-format.mjs
//
// The per-release rows printed `toFixed(2)` regardless of sample size,
// and the fix for that printed `toFixed(0)` — which turned a release
// with 900 sessions and 3 crashes into a flat `100%`. Losing precision
// is a choice; losing the crash is a different statement.

import { formatCrashFree } from '../src/lib/crash-free.ts';

const CASES = [
  // [pct, samples, crashed, want, why]
  [99.67, 900, 3, '99.6', 'a thin sample keeps one decimal and does not round to 100'],
  [99.44, 1240, 7, '99.44', 'a large sample keeps two'],
  [99.999, 100000, 1, '99.99', 'one crash in a hundred thousand is still not 100'],
  [90, 10, 1, '90.0', 'ten sessions, one crashed'],
  [100, 1240, 0, '100.00', 'nothing crashed, so 100 is the true answer'],
  [100, 12, 0, '100.0', 'and at a thin sample it says so with fewer digits'],
];

const bad = [];
for (const [pct, samples, crashed, want, why] of CASES) {
  const got = formatCrashFree(pct, samples, crashed);
  if (got !== want) bad.push(`${pct} over ${samples} with ${crashed} crashed → ${got}, want ${want} (${why})`);
}

// The one that matters most, stated as its own claim: nothing with a
// crash behind it may render as 100 at any precision.
for (const samples of [10, 999, 1000, 500000]) {
  const pct = 100 - 1 / samples;
  const got = formatCrashFree(pct, samples, 1);
  if (Number(got) >= 100) bad.push(`${samples} sessions with 1 crash renders ${got}% — a crash that reads as none`);
}

if (bad.length) {
  for (const b of bad) console.error(`✗ ${b}`);
  process.exit(1);
}
console.log(`✓ ${CASES.length + 4} crash-free renderings keep the crash visible`);
