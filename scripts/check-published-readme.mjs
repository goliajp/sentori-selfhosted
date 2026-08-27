// The README on npm must match the one in this repository.
//
// A clean-context agent was pointed at the npm page as "the current API
// surface, shipped with the package". It was not: the `init()` options
// table — the only prose answer to "what are the defaults" — had been
// added to master and never republished. The agent followed the
// pointer as instructed and got the worse document, then had to guess
// a CDN path to a `.d.ts` to get a trustworthy answer.
//
// Two documents claimed the copies were identical. `diff` said
// otherwise. This is that diff, run by a machine.
//
// Needs the network, so it is not in preflight — it runs before a
// release, where the answer is actionable: either publish, or stop
// telling readers the published copy is current.
//
//   node scripts/check-published-readme.mjs            # warn
//   node scripts/check-published-readme.mjs --strict   # fail
import { readFileSync } from 'node:fs';

const ROOT = new URL('..', import.meta.url).pathname;
const PKGS = [
  ['sdk/react-native', 'README.md'],
  ['sdk/cli', 'README.md'],
];

const strict = process.argv.includes('--strict');
let drifted = 0;
let checked = 0;

for (const [dir, file] of PKGS) {
  let pkg;
  try {
    pkg = JSON.parse(readFileSync(`${ROOT}${dir}/package.json`, 'utf8'));
  } catch {
    console.error(`✗ no package.json in ${dir}. Broken checker.`);
    process.exit(1);
  }
  const local = readFileSync(`${ROOT}${dir}/${file}`, 'utf8');

  const meta = await fetch(
    `https://registry.npmjs.org/${pkg.name.replace('/', '%2f')}`,
  ).then((r) => (r.ok ? r.json() : null)).catch(() => null);
  if (!meta) {
    console.log(`  ? ${pkg.name}: registry unreachable, skipped`);
    continue;
  }
  const latest = meta['dist-tags']?.latest;
  const published = meta.versions?.[latest]?.readme ?? meta.readme;
  checked++;

  if (!published) {
    console.log(`  ? ${pkg.name}@${latest}: registry carries no readme, skipped`);
    continue;
  }

  const norm = (s) => s.replace(/\r\n/g, '\n').trim();
  if (norm(published) === norm(local)) {
    console.log(`  ✓ ${pkg.name}@${latest} matches the repo README`);
    continue;
  }

  drifted++;
  const l = norm(local).split('\n');
  const p = norm(published).split('\n');
  const onlyLocal = l.filter((x) => x.trim() && !p.includes(x)).length;
  const onlyPub = p.filter((x) => x.trim() && !l.includes(x)).length;
  console.error(
    `  ✗ ${pkg.name}@${latest}: the published README differs — ` +
      `${onlyLocal} line(s) only in the repo, ${onlyPub} only on npm`,
  );
  const sample = l.filter((x) => x.trim() && !p.includes(x)).slice(0, 4);
  for (const s of sample) console.error(`      + ${s.slice(0, 76)}`);
  console.error(
    `      Readers are told this copy is current. Either publish ` +
      `${pkg.name}, or stop pointing at npm as the reference.`,
  );
}

if (checked === 0) {
  console.log('  (nothing checked — registry unreachable)');
  process.exit(0);
}
if (drifted > 0 && strict) process.exit(1);
if (drifted > 0) {
  console.log(`\n${drifted} package README(s) drifted. Not fatal here; ` +
    `--strict makes it fatal.`);
}
