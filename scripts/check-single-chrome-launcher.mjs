#!/usr/bin/env node
// There is one way to start a headless Chrome in this repository.
//
//   node scripts/check-single-chrome-launcher.mjs
//
// There were two, written a week apart and not meant to differ. One
// asked the kernel for a port and passed `--no-first-run`; the other
// took a fixed 9555 and passed neither. The first never failed on CI.
// The second failed twice in six runs with "chrome printed nothing at
// all" and the process still alive — which is what both a taken fixed
// port and a first-run prompt look like from outside, and neither
// could happen to the first.
//
// A second launcher is not a style problem. It is a second set of
// flags that nobody compares until one of them is flaky on a machine
// nobody can log into.

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const LAUNCHER = 'scripts/lib/headless-chrome.mjs';

const SEARCH = ['scripts', 'webapp/devtools', 'sdk'];
const SKIP = /node_modules|\/lib\/|\.build|target/;

function walk(dir, out = []) {
  let entries;
  try {
    entries = readdirSync(dir);
  } catch {
    return out;
  }
  for (const e of entries) {
    const p = join(dir, e);
    if (SKIP.test(p)) continue;
    if (statSync(p).isDirectory()) walk(p, out);
    else if (/\.(mjs|js|ts)$/.test(p)) out.push(p);
  }
  return out;
}

const files = SEARCH.flatMap((d) => walk(join(root, d)));
if (files.length === 0) {
  console.error('✗ no sources were read — this checker checked nothing');
  process.exit(1);
}

// Spawning something that is Chrome. A `spawn(` alone is fine; what is
// not is a second place deciding how a browser starts.
const CHROME_SPAWN = /spawn\s*\(\s*[A-Za-z_$][\w$]*\s*,\s*\[[^\]]*--headless/s;

const offenders = [];
for (const f of files) {
  const rel = relative(root, f);
  if (rel === LAUNCHER) continue;
  const src = readFileSync(f, 'utf8');
  if (CHROME_SPAWN.test(src)) offenders.push(rel);
}

if (offenders.length > 0) {
  console.error('✗ a second headless-Chrome launcher:');
  for (const o of offenders) console.error(`    ${o}`);
  console.error(
    `\nUse \`launchChrome\` from ${LAUNCHER}. Two launchers means two flag\n` +
      'sets, and nobody compares them until one is flaky on a machine\n' +
      'nobody can log into.',
  );
  process.exit(1);
}

// And the one that exists has to still be the one that exists.
const launcher = readFileSync(join(root, LAUNCHER), 'utf8');
for (const flag of [
  '--headless=new',
  '--remote-debugging-port=0',
  '--no-first-run',
  '--no-default-browser-check',
  '--disable-gpu',
  '--no-sandbox',
  '--disable-dev-shm-usage',
]) {
  // As an argument, not as prose. The header documents every flag and
  // why it is there, so `includes(flag)` matched the explanation of a
  // flag that had been deleted from the array — the probe removed the
  // argument and this stayed green.
  if (!launcher.includes(`'${flag}'`)) {
    console.error(`✗ ${LAUNCHER} no longer passes ${flag}`);
    console.error('  Each of those is there for a failure that happened. Read the header.');
    process.exit(1);
  }
}

console.log(`✓ one headless-Chrome launcher, across ${files.length} files`);
