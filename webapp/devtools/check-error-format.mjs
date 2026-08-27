// The dashboard's default error text must name what went wrong.
//
// `formatApiError` is the third argument every `useAsyncData` call site
// leaves out — all 24 of them — so it is the sentence a user reads when
// anything in the dashboard fails. It shipped as `` `: ` ``: a template
// literal whose holes had been emptied, most likely when ApiError's
// `body` was renamed to `message`. Nothing caught it. It type-checks, it
// lints, and no sweep renders an error state, so for as long as the
// history goes back every API failure in the console displayed as a bare
// colon and a space.
//
// This runs the real function against a real ApiError and requires the
// status and the message to survive into the output. It deliberately
// does not compare against a fixed string — the wording is free to
// change, the two facts are not.
import { readFileSync } from 'node:fs';

const SRC = 'src/lib/useAsyncData.ts';
const body = readFileSync(new URL(`../${SRC}`, import.meta.url), 'utf8');

const m = body.match(
  /export function formatApiError\(e: unknown\): string \{\s*return ([^;]+);\s*\}/,
);
if (!m) {
  console.error(
    `✗ could not find formatApiError in ${SRC}. This checker is broken, ` +
      `not the tree — it has nothing to judge and must not pass.`,
  );
  process.exit(1);
}

// Evaluate the returned expression with a stand-in ApiError, so the
// check is on behaviour rather than on the shape of the source.
class ApiError extends Error {
  constructor(status, message) {
    super(message);
    this.status = status;
  }
}
const expr = m[1].replace(/\s+/g, ' ');
let out;
try {
  out = new Function('e', 'ApiError', `return ${expr};`)(
    new ApiError(503, 'the database refused the connection'),
    ApiError,
  );
} catch (err) {
  console.error(`✗ formatApiError threw: ${err.message}`);
  process.exit(1);
}

const missing = [];
if (!String(out).includes('503')) missing.push('the status code');
if (!String(out).includes('the database refused the connection'))
  missing.push("the server's message");

if (missing.length > 0) {
  console.error(`✗ ${SRC}: formatApiError drops ${missing.join(' and ')}.`);
  console.error(`    it returned: ${JSON.stringify(out)}`);
  console.error(
    '  This is the text a user reads when the dashboard fails. It must ' +
      'say what failed, not punctuation.',
  );
  process.exit(1);
}

console.log(`✓ formatApiError names both parts: ${JSON.stringify(out)}`);
