// Every entry point a package advertises must exist after a build.
//
// `@goliapkg/sentori-react-native@5.7.0` — the current, published,
// primary SDK — declared three subpath exports whose files were not in
// the tarball and whose sources were not in the repo: `./compat`
// (a Sentry compatibility layer), `./expo-compat`, and `./feedback`.
// They went with the v1 redesign; the export map did not. Anyone
// following the old docs into
// `import * as Sentry from '@goliapkg/sentori-react-native/compat'`
// got a resolve failure from a package that installs cleanly.
//
// Nothing catches this: `tsc` never looks at the export map, `npm
// pack` does not verify targets, and the failure appears in someone
// else's bundler.
//
// Run after `build:sdks`, which is when the answer is meaningful —
// before it, every `lib/` target is legitimately absent.
//
//   node scripts/check-package-entrypoints.mjs

import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

const SDK_DIR = 'sdk';

const packages = readdirSync(SDK_DIR).filter((d) =>
  existsSync(join(SDK_DIR, d, 'package.json')),
);
if (packages.length === 0) {
  console.error(`✗ no packages under ${SDK_DIR}/ — this checker read nothing.`);
  process.exit(1);
}

const problems = [];
let checked = 0;

/** Every `./…` path a package.json points at, with the field it came from. */
function targets(pkg) {
  const out = [];
  const push = (field, value) => {
    if (typeof value === 'string' && value.startsWith('.')) out.push([field, value]);
  };
  push('main', pkg.main);
  push('types', pkg.types);
  push('module', pkg.module);
  for (const [name, value] of Object.entries(pkg.bin ?? {})) push(`bin.${name}`, value);
  for (const [sub, value] of Object.entries(pkg.exports ?? {})) {
    if (typeof value === 'string') push(`exports["${sub}"]`, value);
    else
      for (const [cond, target] of Object.entries(value ?? {}))
        push(`exports["${sub}"].${cond}`, target);
  }
  return out;
}

for (const name of packages) {
  const dir = join(SDK_DIR, name);
  const pkg = JSON.parse(readFileSync(join(dir, 'package.json'), 'utf8'));
  for (const [field, target] of targets(pkg)) {
    checked += 1;
    if (!existsSync(join(dir, target))) {
      problems.push(`${dir}/package.json ${field} → ${target} does not exist`);
    }
  }
}

if (problems.length === 0) {
  console.log(`✓ ${checked} entry points across ${packages.length} packages, all present`);
  process.exit(0);
}
for (const p of problems) console.error(`✗ ${p}`);
console.error(
  '\nAn export map is a promise about what an import resolves to. Remove the\n' +
    'entry, or build the file — a package that installs cleanly and fails at\n' +
    'import is worse than one that fails to install.',
);
process.exit(1);
