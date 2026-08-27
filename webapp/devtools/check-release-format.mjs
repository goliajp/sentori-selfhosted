// `formatReleaseIn` must keep the build suffix when it is the only
// thing telling two rows apart.
//
// An issue's release breakdown drew `reg@1.0.0` on two rows with
// different event counts and one of them marked fixed. The rows were
// `reg@1.0.0+1` and `reg@1.0.0+2`. Dropping the suffix is right in
// general — on iOS it is noise — but in a list it erased the one
// question that panel answers.
import { readFileSync } from 'node:fs';

const SRC = 'src/components/ui.tsx';
const src = readFileSync(new URL(`../${SRC}`, import.meta.url), 'utf8');

const grab = (name) => {
  const i = src.indexOf(`export function ${name}(`);
  if (i < 0) return null;
  let depth = 0, start = src.indexOf('{', i), k = start;
  for (; k < src.length; k++) {
    if (src[k] === '{') depth++;
    else if (src[k] === '}' && --depth === 0) break;
  }
  // Strip the TypeScript annotations so `new Function` can take it —
  // evaluating the real source is the point, so it is stripped rather
  // than restated.
  return src
    .slice(i, k + 1)
    .replace(/^export /, '')
    .replace(/:\s*string\[\]/g, '')
    .replace(/:\s*string(?=[),])/g, '')
    .replace(/\)\s*:\s*string\s*\{/, ') {');
};

const bodies = ['formatRelease', 'formatReleaseIn'].map(grab);
if (bodies.some((b) => !b)) {
  console.error(
    `✗ could not find formatRelease/formatReleaseIn in ${SRC}. This ` +
      `checker is broken, not the tree.`,
  );
  process.exit(1);
}

const fn = new Function(
  `${bodies.join('\n')}\nreturn { formatRelease, formatReleaseIn };`,
)();

const CASES = [
  ['single release keeps it short', 'app@1.2.3+45', ['app@1.2.3+45'], 'app@1.2.3'],
  ['two builds of one version keep the suffix', 'reg@1.0.0+1',
    ['reg@1.0.0+1', 'reg@1.0.0+2'], 'reg@1.0.0+1'],
  ['the other one too', 'reg@1.0.0+2',
    ['reg@1.0.0+1', 'reg@1.0.0+2'], 'reg@1.0.0+2'],
  ['distinct versions stay short', 'app@1.0.0+1',
    ['app@1.0.0+1', 'app@2.0.0+1'], 'app@1.0.0'],
  ['no suffix at all', 'app@1.0.0', ['app@1.0.0', 'app@2.0.0'], 'app@1.0.0'],
  ['identical strings are not a collision', 'app@1.0.0+1',
    ['app@1.0.0+1', 'app@1.0.0+1'], 'app@1.0.0'],
];

let failed = 0;
for (const [name, release, all, want] of CASES) {
  const got = fn.formatReleaseIn(release, all);
  if (got !== want) {
    console.error(`✗ ${name}: formatReleaseIn(${release}) → ${got}, want ${want}`);
    failed++;
  }
}
// The collision case must actually produce two distinct labels.
const labels = ['reg@1.0.0+1', 'reg@1.0.0+2'].map((r) =>
  fn.formatReleaseIn(r, ['reg@1.0.0+1', 'reg@1.0.0+2']),
);
if (labels[0] === labels[1]) {
  console.error(`✗ two builds still render identically: ${labels[0]}`);
  failed++;
}

if (failed > 0) process.exit(1);
console.log(`✓ ${CASES.length} release-label assertions; two builds cannot collide`);
