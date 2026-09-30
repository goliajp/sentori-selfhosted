// The version an SDK reports is the version it is.
//
// Every SDK puts `SDK_VERSION` in the `Sentori-Sdk` header, and that
// string is what the server records and what a support conversation
// starts from. It is a hand-written constant beside the package.json
// it has to agree with, so it goes stale on exactly the commit nobody
// is looking at it: the release.
//
// `sdk/react-native` had a test asserting the two match. Nothing did
// for the other packages, so `sentori-web` and `sentori-weapp` were
// published at 1.0.0, 1.0.1 and 1.0.2 while still reporting `0.1.0` on
// every event they sent.
//
// A repo-level gate rather than a per-package test, because the
// per-package version is what was missing.
//
//   node scripts/check-sdk-version-constant.mjs

import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

const ROOT = new URL('..', import.meta.url).pathname;
const SDK = join(ROOT, 'sdk');

const problems = [];
let checked = 0;

for (const dir of readdirSync(SDK)) {
  const pkgFile = join(SDK, dir, 'package.json');
  const src = join(SDK, dir, 'src', 'transport.ts');
  if (!existsSync(pkgFile) || !existsSync(src)) continue;
  const declared = /SDK_VERSION\s*=\s*'([^']+)'/.exec(readFileSync(src, 'utf8'));
  if (!declared) continue;
  checked += 1;
  const version = JSON.parse(readFileSync(pkgFile, 'utf8')).version;
  if (declared[1] !== version) {
    problems.push(
      `sdk/${dir}: SDK_VERSION says '${declared[1]}', package.json says '${version}' — ` +
        'every event this package sends reports the wrong version',
    );
  }
}

if (checked < 3) {
  console.error(
    `✗ found SDK_VERSION in ${checked} package(s). Either it was renamed or the ` +
      'transports moved — this checker is reading nothing and must not pass.',
  );
  process.exit(1);
}

if (problems.length > 0) {
  console.error(`✗ ${problems.length} SDK(s) reporting a version they are not:`);
  for (const p of problems) console.error(`    ${p}`);
  process.exit(1);
}

console.log(`✓ ${checked} SDKs report the version their package.json declares`);
