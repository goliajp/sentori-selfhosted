// A CLI command printed in the docs has to work when it is copied.
//
//   node scripts/check-doc-commands.mjs
//
// Two ways this failed on one page, on the same day it was written:
//
//   - `--token "$SENTORI_API_TOKEN"`, a variable nothing reads. The
//     CLI takes `$SENTORI_TOKEN` or `$SENTORI_ADMIN_TOKEN`, so a
//     reader who exported the documented name got `--token is
//     required` with the value sitting in the environment.
//   - an upload with no `--api-url`, which defaults to GOLIA's own
//     instance. A self-hosted customer's dSYM leaves their build
//     machine, goes somewhere that is not theirs, and the command
//     exits 0 because uploads never fail a build.
//
// `check-env-vars-real.mjs` did not catch the first: its rule is that
// some code reads the variable, and our own e2e script does. Reading
// it in a test harness is not the same as the CLI reading it.

import { readFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';

const CLI = 'sdk/cli/src/index.ts';
const cli = readFileSync(CLI, 'utf8');

// What the CLI really falls back to, read out of the CLI.
const tokenVars = [...new Set([...cli.matchAll(/process\.env\.(SENTORI_\w*TOKEN)/g)].map((m) => m[1]))];
const urlVars = [...new Set([...cli.matchAll(/process\.env\.(SENTORI_\w*URL)/g)].map((m) => m[1]))];
const problems = [];
if (tokenVars.length === 0 || urlVars.length === 0) {
  console.error(`✗ could not read the env fallbacks out of ${CLI} — this check now reads nothing`);
  process.exit(1);
}

const docs = execFileSync('git', ['ls-files', 'docs/**/*.md', 'docs/*.md'], { encoding: 'utf8' })
  .split('\n')
  .filter(Boolean);

for (const file of docs) {
  const src = readFileSync(file, 'utf8');

  // A token passed to the CLI must be a name the CLI reads.
  for (const m of src.matchAll(/--token\s+"\$(\w+)"/g)) {
    if (!tokenVars.includes(m[1])) {
      problems.push(
        `${file}: \`--token "$${m[1]}"\` names a variable the CLI does not read ` +
          `(it reads ${tokenVars.join(' or ')}) — the command fails with the value set`,
      );
    }
  }

  // Every upload/check command must name the instance. The default
  // is ours, and a self-hoster shipping symbols to it is a privacy
  // failure that exits 0.
  //
  // Scanned per fenced block, because a CI recipe sets the variable
  // in the job's `env:` above the command it applies to — and because
  // a page-wide search is too generous the other way: three recipes
  // set `SENTORI_INGEST_URL`, which the CLI does not read, while a
  // paragraph elsewhere mentioned the right name.
  for (const block of src.split('```').filter((_, i) => i % 2 === 1)) {
    const cmd = /sentori-cli(?:@latest)?\s+(upload|artifacts)\s+(\w+)/.exec(block);
    if (!cmd) continue;
    const namesTheInstance =
      block.includes('--api-url') ||
      block.includes('--ingest-url') ||
      urlVars.some((v) => block.includes(v));
    if (!namesTheInstance) {
      problems.push(
        `${file}: \`${cmd[1]} ${cmd[2]}\` is in a block that names neither --api-url nor ` +
          `${urlVars.join(' / ')} — the CLI then defaults to GOLIA's own instance, so a ` +
          `self-hosted reader uploads their symbols to us and the command exits 0`,
      );
    }
  }
}

// And the same trap one layer up. `ingestUrl` in an SDK example is
// where a reader's crashes go, and the quickstart pointed at
// `sentori.golia.jp` — ours. Copy-pasting the getting-started block
// sent an app's crash stream to strangers, which `protocol.md`
// describes as the thing that must not happen.
const OURS = 'sentori.golia.jp';
for (const file of docs) {
  const src = readFileSync(file, 'utf8');
  for (const m of src.matchAll(/ingestUrl\s*[:=]\s*['"]https?:\/\/([^'"\/]+)/g)) {
    if (m[1].endsWith(OURS)) {
      problems.push(
        `${file}: an SDK example sets ingestUrl to ${m[1]} — that is our instance, so a ` +
          `reader who copies the block sends their app's crashes to us`,
      );
    }
  }
}

if (problems.length === 0) {
  console.log(
    `✓ documented commands and SDK examples name the reader's own instance`,
  );
  process.exit(0);
}
for (const p of problems) console.error(`✗ ${p}`);
process.exit(1);
