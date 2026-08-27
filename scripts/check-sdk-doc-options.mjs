// Does the SDK reference document the options the SDK actually has?
//
// `sdk/react-native/README.md` is the page an integrator reads —
// it ships with the package and is what npm renders. It
// carried a table describing a v0.2-era API for months: `capture`
// with `globalErrors` / `promiseRejections` / `network` (no such
// option), `release` marked required (it is optional), `environment`
// defaulting to `'dev'`/`'prod'` (it is `'production'`), and
// `ingestUrl` marked optional with a default pointing at somebody
// else's server (it is required, and there is no default). Meanwhile
// `replaySeconds`, `replayScreens`, `backendHealthUrl`, `logLevel`,
// `beforeSend` and every `detect` toggle went unmentioned.
//
// None of that is visible in a diff: the type changes in one file and
// the table stays put in another. So this compares the two.
//
// It checks membership, not prose — an option in the type must appear
// in the table and vice versa. Types, defaults and wording stay a
// human's job; naming is what silently drifts.
//
//   node scripts/check-sdk-doc-options.mjs

import { readFileSync } from 'node:fs';

const TYPES = 'sdk/core/src/types.ts';
// Was docs/sdk-react-native.md until 2026-08-27, when that page was
// archived: it described the pre-v1 API and said so in its own first
// line, while this gate held it to the current InitConfig.
const DOC = 'sdk/react-native/README.md';

/** Field names of `export type InitConfig = { ... }`, top level only. */
function initConfigFields(src) {
  const start = src.indexOf('export type InitConfig = {');
  if (start < 0) return null;
  const fields = [];
  let depth = 0;
  for (let i = src.indexOf('{', start); i < src.length; i++) {
    const c = src[i];
    if (c === '{') depth += 1;
    else if (c === '}') {
      depth -= 1;
      if (depth === 0) break;
    } else if (depth === 1 && /[A-Za-z_]/.test(c)) {
      // A field starts at the beginning of a line at depth 1.
      const lineStart = src.lastIndexOf('\n', i) + 1;
      if (src.slice(lineStart, i).trim() !== '') continue;
      const m = /^([A-Za-z_$][\w$]*)\??\s*:/.exec(src.slice(i));
      if (m) fields.push(m[1]);
    }
  }
  return [...new Set(fields)];
}

/** Option names in the doc's first `| Option |` table. */
function documentedOptions(doc) {
  const head = doc.indexOf('| Option | Type | Required | Default |');
  if (head < 0) return null;
  const names = [];
  for (const line of doc.slice(head).split('\n').slice(2)) {
    if (!line.startsWith('|')) break;
    const m = /^\|\s*`([^`]+)`/.exec(line);
    if (m) names.push(m[1]);
  }
  return names;
}

const fields = initConfigFields(readFileSync(TYPES, 'utf8'));
if (!fields || fields.length === 0) {
  console.error(
    `✗ could not read InitConfig fields from ${TYPES} — the type moved or ` +
      'was renamed. A checker that parses nothing must not pass.',
  );
  process.exit(1);
}
const documented = documentedOptions(readFileSync(DOC, 'utf8'));
if (!documented || documented.length === 0) {
  console.error(
    `✗ could not find the option table in ${DOC} — its header changed. ` +
      'A checker that parses nothing must not pass.',
  );
  process.exit(1);
}

const missing = fields.filter((f) => !documented.includes(f));
const extra = documented.filter((d) => !fields.includes(d));

if (missing.length === 0 && extra.length === 0) {
  console.log(`✓ ${fields.length} init options, all documented`);
  process.exit(0);
}
for (const f of missing) {
  console.error(`✗ ${DOC} does not document \`${f}\` — an option nobody can find`);
}
for (const d of extra) {
  console.error(`✗ ${DOC} documents \`${d}\`, which InitConfig does not have`);
}
console.error(
  `\n${TYPES} is the source of truth. Update the table, or the type if the ` +
    'doc is describing what the option should have been called.',
);
process.exit(1);
