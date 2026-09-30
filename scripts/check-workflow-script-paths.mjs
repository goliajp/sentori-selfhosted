#!/usr/bin/env node
// A workflow that runs a script must be triggered by changes to it.
//
//   node scripts/check-workflow-script-paths.mjs
//
// `mobile-e2e` runs `scripts/ios-crash-loop.sh`, and its `on.paths`
// did not mention the file. So editing the rig that crashes a real app
// and reads the report back triggered nothing — the gate could be
// broken, or fixed, and no run would say either way. Both happened:
// a fix for it sat unverified because the only workflow that executes
// it never fired.
//
// Two levels have to agree. `on.paths` decides whether the workflow
// runs at all; a `dorny/paths-filter` inside it decides which jobs do.
// A script named in neither is a gate nobody can retest.
//
// This reads the literal strings rather than a YAML parser on purpose:
// the question is "does the trigger list mention this path", and a
// glob in a list is exactly what a trigger list holds.

import { readFileSync, readdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const dir = join(root, '.github/workflows');

/** Does a trigger glob cover this path? */
const covers = (glob, path) => {
  if (glob === path) return true;
  // `a/**` covers `a/anything`, and `a/**` also covers `a` itself.
  if (glob.endsWith('/**')) {
    const prefix = glob.slice(0, -3);
    return path === prefix || path.startsWith(`${prefix}/`);
  }
  if (glob.endsWith('/*')) {
    const prefix = glob.slice(0, -2);
    return path.startsWith(`${prefix}/`) && !path.slice(prefix.length + 1).includes('/');
  }
  return false;
};

const problems = [];
const unfiltered = [];
let checked = 0;

for (const file of readdirSync(dir).filter((f) => f.endsWith('.yml'))) {
  const src = readFileSync(join(dir, file), 'utf8');

  // Workflows with no push trigger (manual, scheduled, called) have
  // nothing to be triggered by, so there is nothing to check.
  if (!/^\s*push:/m.test(src)) continue;

  // The `push:` trigger's paths, and only those.
  //
  // Two reasons it is not the whole `on:` block. A script listed in a
  // job's paths-filter but not in a trigger is unreachable — the
  // filter decides which jobs run *given* that the workflow ran, and
  // it never does. And these workflows carry a `pull_request:` block
  // with its own copy of the list, which in this repository never
  // fires: work lands by `git merge --no-ff` onto develop and no pull
  // request is opened. Counting that copy made this checker green
  // while the `push:` list was missing the script — which is the
  // exact hole it was written to find, reproduced inside itself.
  const pushAt = src.search(/^\s{2}push:/m);
  if (pushAt === -1) continue;
  const after = src.slice(pushAt + 7);
  const end = after.search(/^\s{2}[a-z_]+:/m);
  const pushBlock = src.slice(pushAt, end === -1 ? src.length : pushAt + 7 + end);
  const declared = [...pushBlock.matchAll(/^\s*-\s*'([^']+)'\s*$/gm)].map((m) => m[1]);
  // No `paths:` means every push runs it, so every script it holds is
  // already reachable. Named in the output rather than skipped in
  // silence: "8 pairs checked" would otherwise read as "all of them".
  if (declared.length === 0) {
    unfiltered.push(file);
    continue;
  }

  const used = new Set(
    [...src.matchAll(/(?:bash|node|sh)\s+(scripts\/[A-Za-z0-9/_.-]+)/g)].map((m) => m[1]),
  );

  for (const script of used) {
    checked += 1;
    if (!declared.some((g) => covers(g, script))) {
      problems.push(
        `${file} runs ${script} and no trigger path mentions it — ` +
          'editing that script fires nothing, so the gate cannot be retested',
      );
    }
  }
}

if (checked === 0) {
  console.error('✗ no workflow/script pairs were checked — this checker read nothing');
  process.exit(1);
}
if (problems.length > 0) {
  for (const p of problems) console.error(`✗ ${p}`);
  console.error(
    '\nAdd the script to the workflow’s `on.paths`, and to the paths-filter of the\n' +
      'job that runs it, or that job stays skipped when the script changes.',
  );
  process.exit(1);
}
console.log(
  `✓ ${checked} workflow/script pairs: every script fires the workflow that runs it` +
    (unfiltered.length > 0
      ? ` (${unfiltered.join(', ')} run on every push, so nothing to filter)`
      : ''),
);
