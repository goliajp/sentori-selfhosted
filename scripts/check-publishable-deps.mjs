// A published package may not declare a workspace protocol.
//
// `workspace:*` resolves inside this monorepo and nowhere else. npm
// installs it verbatim, so a consumer running `npm install` gets
//
//   npm error code EUNSUPPORTEDPROTOCOL
//   npm error Unsupported URL Type "workspace:": workspace:*
//
// and the package is simply unobtainable. `@goliapkg/sentori-web@1.0.0`
// and `@goliapkg/sentori-weapp@1.0.0` went out that way — `npm publish`
// reported success, the registry accepted them, and the first install
// from outside failed. `sdk/react-native` had it right all along with
// `^3.0.0`; the two new packages were never set up the same way and
// nothing compared them.
//
// devDependencies are exempt: a consumer never installs them.
//
//   node scripts/check-publishable-deps.mjs

import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

const ROOT = new URL('..', import.meta.url).pathname;
const SHIPPED = ['dependencies', 'peerDependencies', 'optionalDependencies'];

const problems = [];
let checked = 0;

for (const pkg of readdirSync(join(ROOT, 'sdk'))) {
  const file = join(ROOT, 'sdk', pkg, 'package.json');
  if (!existsSync(file)) continue;
  const json = JSON.parse(readFileSync(file, 'utf8'));
  if (json.private) continue;
  checked += 1;
  for (const field of SHIPPED) {
    for (const [name, range] of Object.entries(json[field] ?? {})) {
      if (String(range).startsWith('workspace:')) {
        problems.push(`sdk/${pkg}/package.json: ${field}.${name} = "${range}"`);
      }
    }
  }
}

if (checked < 4) {
  console.error(`✗ inspected ${checked} publishable packages. Broken checker, not a broken tree.`);
  process.exit(1);
}

if (problems.length > 0) {
  console.error(`✗ ${problems.length} workspace range(s) in what consumers install:`);
  for (const p of problems) console.error(`    ${p}`);
  console.error(
    '  npm installs these verbatim and the install fails with ' +
      'EUNSUPPORTEDPROTOCOL. Name a real range, as sdk/react-native does.',
  );
  process.exit(1);
}

console.log(`✓ ${checked} publishable packages, no workspace protocol in what ships`);
