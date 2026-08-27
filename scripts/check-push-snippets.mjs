// The server snippets the console hands out, checked against the
// server that has to accept them.
//
// A snippet is documentation that someone compiles. The last time
// this repo shipped a command it had not run, the command POSTed to a
// route no server had ever served and the docs taught it for a month.
//
// Two checks, because two are possible:
//
//   1. Every snippet names the route the server actually registers,
//      and the three field names the handler actually reads. A
//      renamed field is a support ticket that opens with "your docs
//      are wrong".
//   2. The two that need no toolchain — Node and Python — are run for
//      real against a server, when one is given. `SENTORI_BASE` and
//      `SENTORI_API_TOKEN` turn that half on; without them this says
//      so rather than reporting a pass it did not earn.

import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const src = readFileSync(join(root, 'webapp/src/lib/push-snippets.ts'), 'utf8');

const problems = [];

// ── 1. the route and the fields, in every snippet ──────────────────

// Read them out of the server rather than restating them here: a
// constant copied into a checker drifts with the thing it checks.
const routes = readFileSync(join(root, 'self-hosted/server/src/handlers/mod.rs'), 'utf8');
for (const path of ['/v1/push/sends', '/v1/push/audience/count']) {
  if (!routes.includes(`"${path}"`)) {
    problems.push(`the server does not register ${path} — the snippets teach a 404`);
  }
}

const LANGS = ['go', 'rust', 'java', 'node', 'python', 'csharp', 'cpp'];
const bodies = src.slice(src.indexOf('const BODIES'));
for (const lang of LANGS) {
  const start = bodies.indexOf(`  ${lang}: (base) =>`);
  if (start === -1) {
    problems.push(`no snippet for ${lang}`);
    continue;
  }
  const next = LANGS.map((l) => bodies.indexOf(`  ${l}: (base) =>`, start + 1))
    .filter((i) => i > start)
    .sort((a, b) => a - b)[0];
  const body = bodies.slice(start, next === undefined ? undefined : next);

  // `${SEND_PATH}` is the interpolation; a literal path would be the
  // copy that drifts.
  if (!body.includes('${SEND_PATH}')) {
    problems.push(`${lang}: does not build its URL from SEND_PATH`);
  }
  for (const field of ['appUserId', 'payload', 'title']) {
    if (!body.includes(field)) problems.push(`${lang}: never mentions \`${field}\``);
  }
  // Case-insensitive: reqwest spells it `bearer_auth`, and a
  // checker that only knows one spelling reports a real snippet as
  // broken.
  if (!/bearer/i.test(body)) problems.push(`${lang}: does not send a bearer token`);
  if (!/\bst_/.test(body)) problems.push(`${lang}: does not show which token to use`);
}

// The handler has to read what they all send.
const send = readFileSync(
  join(root, 'self-hosted/server/src/handlers/sdk/push/send.rs'),
  'utf8',
);
for (const field of ['app_user_id', 'payload']) {
  if (!send.includes(field)) {
    problems.push(`the send handler no longer reads \`${field}\`, which every snippet sends`);
  }
}

// ── 2. run the two that can be run ─────────────────────────────────

const base = process.env.SENTORI_BASE;
const token = process.env.SENTORI_API_TOKEN;
let ran = 0;

if (base && token) {
  const dir = mkdtempSync(join(tmpdir(), 'sentori-snippets-'));

  // Extracted the way a reader would: take the snippet, put the real
  // base and token in, call the function it defines.
  //
  // Run as TypeScript, unmodified. It was two regexes that knew about
  // `: string` and `Promise<void>`, and the first change to a return
  // type left the runner handing itself a file that could not parse.
  // Then it was bun, which is not on the PATH of the job that runs the
  // e2e. Node strips the types itself, and node is what runs this.
  const nodeSrc = renderFor('node', base).replace("'st_…'", JSON.stringify(token));
  writeFileSync(
    join(dir, 'notify.ts'),
    `${nodeSrc}\n` +
      `const id = await notify('snippet-check', 'snippet', 'from the checker')\n` +
      `if (typeof id !== 'string' || id.length === 0) throw new Error('no sendId')\n`,
  );
  run('node', ['node', '--experimental-strip-types', join(dir, 'notify.ts')]);

  const pySrc = renderFor('python', base).replace('"st_…"', JSON.stringify(token));
  writeFileSync(
    join(dir, 'notify.py'),
    `${pySrc}\n\n` +
      `send_id = notify("snippet-check", "snippet", "from the checker")\n` +
      `assert isinstance(send_id, str) and send_id, "no sendId"\n`,
  );
  run('python', ['python3', join(dir, 'notify.py')]);
}

function renderFor(lang, url) {
  const start = bodies.indexOf(`  ${lang}: (base) =>`);
  const open = bodies.indexOf('`', start);
  // Scan for the closing backtick rather than searching for "`,": the
  // Node snippet contains an escaped backtick followed by a comma, so
  // the search stopped seven lines in and handed the runner a file
  // that could not parse.
  let close = open + 1;
  while (close < bodies.length) {
    if (bodies[close] === '\\') {
      close += 2;
      continue;
    }
    if (bodies[close] === '`') break;
    close += 1;
  }
  return bodies
    .slice(open + 1, close)
    .replaceAll('${base}', url)
    .replaceAll('${SEND_PATH}', '/v1/push/sends')
    .replaceAll('\\`', '`')
    .replaceAll('\\${', '${');
}

function run(label, argv) {
  try {
    execFileSync(argv[0], argv.slice(1), { encoding: 'utf8', stdio: 'pipe' });
    ran += 1;
  } catch (e) {
    problems.push(`${label}: the snippet did not run — ${e.stderr || e.message}`);
  }
}

if (problems.length > 0) {
  console.error('✗ push snippets:\n');
  for (const p of problems) console.error(`    ${p}`);
  process.exit(1);
}

const note = base && token ? `${ran} run against ${base}` : 'not run — set SENTORI_BASE and SENTORI_API_TOKEN';
console.log(`✓ ${LANGS.length} push snippets name the route and the fields; ${note}`);
