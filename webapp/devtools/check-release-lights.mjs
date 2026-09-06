// The dot on a collapsed release row.
//
// `focus-ai-app@5.4.26081201+383` carried a working
// `insight-android-bundle.map` and, beside it, an `index.android.bundle`
// somebody had uploaded as a source map. The good one lit the dot
// green. The broken one was reachable only by expanding the row, and
// nobody expands a row that looks fine.
//
// The case below that would have caught it is `covered + broken`.
// Everything else is here so the fix cannot be "make it amber more
// often" — an amber that fires on a healthy release is an amber
// people learn to scroll past.

import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const { lightColour, lightState } = await import(join(root, 'src/lib/release-lights.ts'));

const problems = [];
let checked = 0;

function is(name, actual, expected) {
  checked += 1;
  if (actual !== expected) {
    problems.push(`${name}: got ${JSON.stringify(actual)}, want ${JSON.stringify(expected)}`);
  }
}

// --- the case this file exists for --------------------------------
is(
  'covered, and something under it does not parse',
  lightState({ broken: true, on: true, used: true }),
  'broken',
);
is(
  '...and it is not green',
  lightColour(lightState({ broken: true, on: true, used: true })) ===
    lightColour('ok'),
  false,
);

// --- the healthy row stays quiet ----------------------------------
is('covered and clean', lightState({ broken: false, on: true, used: true }), 'ok');
is(
  'covered and clean on a platform this release does not use',
  lightState({ broken: false, on: true, used: false }),
  'ok',
);

// --- absence, which already worked --------------------------------
is('missing on a platform in use', lightState({ broken: false, on: false, used: true }), 'missing');
is('absent on a platform not in use', lightState({ broken: false, on: false, used: false }), 'unused');

// Nothing usable AND something unreadable: somebody uploaded for this
// kind and got nothing out of it. Amber would under-state it.
is(
  'nothing usable, and what was uploaded is unreadable',
  lightState({ broken: true, on: false, used: false }),
  'missing',
);

// --- before the artifacts load ------------------------------------
is('not loaded yet', lightState({ broken: false, on: undefined, used: true }), 'unknown');
is(
  'not loaded yet reads as quiet, not as missing',
  lightColour(lightState({ broken: false, on: undefined, used: true })) ===
    lightColour('missing'),
  false,
);

// --- the four states are four colours -----------------------------
const seen = new Map();
for (const s of ['ok', 'broken', 'missing', 'unused', 'unknown']) {
  checked += 1;
  const c = lightColour(s);
  // `unused` and `unknown` deliberately share the quiet colour; the
  // other three must be distinguishable or the row cannot be read.
  if (!['unused', 'unknown'].includes(s)) {
    if (seen.has(c)) problems.push(`${s} and ${seen.get(c)} render the same colour`);
    seen.set(c, s);
  }
}

if (problems.length > 0) {
  console.error('✗ release lights:\n');
  for (const p of problems) console.error(`    ${p}`);
  process.exit(1);
}
console.log(`✓ ${checked} light-state assertions; a broken artifact cannot read as green`);
