// Run the curl commands the documentation hands out, against a live
// instance, and require them to work.
//
//   SENTORI_BASE=… SENTORI_TOKEN=… node scripts/check-docs-curl.mjs
//
// docs/getting-started.md carried a "try it without an SDK" request
// for as long as the history goes back. It sent `timestamp` where the
// server requires `occurredAt`, and put `error` at the top level where
// the server wants it inside `payload`. It answered `422 missing field
// occurredAt` — the first thing a careful reader tries, and the first
// thing an agent copies, and it had never once been executed.
//
// The command is extracted from the markdown rather than restated
// here, so this cannot pass while the page is wrong: the text in the
// fence IS the thing under test.
import { readFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';

const BASE = process.env.SENTORI_BASE;
const TOKEN = process.env.SENTORI_TOKEN;
if (!BASE || !TOKEN) {
  console.error('set SENTORI_BASE and SENTORI_TOKEN');
  process.exit(2);
}

const ROOT = new URL('..', import.meta.url).pathname;
const DOC = 'docs/getting-started.md';
const text = readFileSync(ROOT + DOC, 'utf8');

// every ```bash fence holding a curl to the ingest endpoint
// Two shapes, two verdicts. The validate endpoint answers 200 with
// `ok: true`; ingest answers 202 with a receipt. Filtering on
// "/v1/events" alone caught both and then failed the validate one for
// not being 202 — which is the checker being wrong, not the page.
const all = [...text.matchAll(/```bash\n([\s\S]*?)\n```/g)].map((m) => m[1]);
const commands = all
  .filter((c) => /^curl /.test(c) && /\/v1\/events(?!\/validate)/.test(c))
  .map((c) => ({ cmd: c, want: 202, kind: 'ingest' }))
  .concat(
    all
      .filter((c) => /^curl /.test(c) && c.includes('/v1/events/validate'))
      .map((c) => ({ cmd: c, want: 200, kind: 'validate' })),
  );

if (commands.length === 0) {
  console.error(
    `✗ found no ingest curl in ${DOC}. This checker is broken, not the ` +
      `tree — it cannot pass on an empty list.`,
  );
  process.exit(1);
}

let failed = 0;
for (const [i, { cmd, want, kind }] of commands.entries()) {
  // `-o /dev/null -w %{http_code}` so the assertion is on the status,
  // and the command otherwise runs exactly as written on the page.
  const runnable = cmd.replace(
    /^curl /,
    'curl -s -o /tmp/sentori-docs-curl.out -w "%{http_code}" ',
  );
  let status = '';
  try {
    status = execFileSync('bash', ['-c', runnable], {
      env: {
        ...process.env,
        SENTORI_INGEST_URL: BASE,
        SENTORI_TOKEN: TOKEN,
      },
      encoding: 'utf8',
    }).trim();
  } catch (e) {
    console.error(`✗ command ${i + 1} in ${DOC} could not run: ${e.message}`);
    failed++;
    continue;
  }

  let body = '';
  try {
    body = readFileSync('/tmp/sentori-docs-curl.out', 'utf8');
  } catch { /* no body */ }

  if (status !== String(want)) {
    console.error(
      `✗ ${DOC}: the ${kind} curl on this page answers ${status}, not ${want}`,
    );
    console.error(`    ${body.slice(0, 200)}`);
    console.error(`  The command, as the page prints it:\n${cmd.replace(/^/gm, '    ')}`);
    failed++;
    continue;
  }
  let parsed;
  try {
    parsed = JSON.parse(body);
  } catch {
    console.error(`✗ ${DOC}: 202 but the body is not JSON: ${body.slice(0, 120)}`);
    failed++;
    continue;
  }
  if (kind === 'ingest') {
    // The page tells the reader to assert on these two.
    for (const k of ['eventId', 'issueId']) {
      if (!parsed[k]) {
        console.error(`✗ ${DOC}: the 202 body has no \`${k}\`, which the page says is the receipt`);
        failed++;
      }
    }
  } else if (parsed.ok !== true) {
    console.error(`✗ ${DOC}: the validate curl parsed but answered ok=${parsed.ok}`);
    failed++;
  }
}

if (failed > 0) process.exit(1);
console.log(
  `✓ ${commands.length} documented curl(s) answer as the page says`,
);
