// One casing on the wire.
//
// `/v1/push` shipped with both: requests were camelCase in all nine
// structs while responses answered `token_id`, `is_new`, `sent_at`,
// `provider_outcome`. The same row read through two routes came back
// with two different sets of field names, and one endpoint took
// `nativeToken` and answered `token_id`.
//
// Nothing catches that by reading code — a snake_case key is a valid
// string. So this reads the keys out of the handlers and says no.
//
// Scoped to `/v1`, the surface other people write against. The admin
// API behind the session cookie is ours on both ends.

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const ROOT = join(root, 'self-hosted/server/src/handlers/sdk');

/** Not field names: SQL fragments, log targets, error codes. */
const NOT_A_FIELD = new Set([
  // Error codes are values, not keys, and read better as snake.
  'not_found',
  'send_not_found',
  'delivery_not_found',
  'invalid_kind',
  'invalid_credential',
  'bad_target',
  'bad_audience',
  'audience_changed',
  'admin_token_required',
  'device_token_not_found_or_revoked',
  'credentials_missing',
]);

function walk(dir, out = []) {
  for (const e of readdirSync(dir)) {
    const p = join(dir, e);
    if (statSync(p).isDirectory()) walk(p, out);
    else if (p.endsWith('.rs')) out.push(p);
  }
  return out;
}

const files = walk(ROOT);
if (files.length === 0) {
  console.error('✗ no handlers found — this checker read nothing and passed');
  process.exit(1);
}

const findings = [];
let keys = 0;

for (const f of files) {
  const src = readFileSync(f, 'utf8');

  // Keys of a `json!` object: `"name":` at the start of a pair.
  for (const m of src.matchAll(/"([a-z][a-zA-Z0-9_]*)"\s*:/g)) {
    const key = m[1];
    if (NOT_A_FIELD.has(key)) continue;
    keys += 1;
    if (key.includes('_')) {
      const line = src.slice(0, m.index).split('\n').length;
      findings.push(`${f.replace(`${root}/`, '')}:${line}  "${key}"`);
    }
  }

  // And the request side: serde has to be told, every time.
  for (const m of src.matchAll(/#\[serde\(rename_all\s*=\s*"([a-zA-Z]+)"\)\]/g)) {
    if (m[1] !== 'camelCase') {
      const line = src.slice(0, m.index).split('\n').length;
      findings.push(`${f.replace(`${root}/`, '')}:${line}  rename_all = "${m[1]}"`);
    }
  }
}

if (findings.length > 0) {
  console.error('✗ /v1 speaks two casings:\n');
  for (const f of findings) console.error(`    ${f}`);
  console.error(
    '\nRequests are camelCase in every struct. A response that answers\n' +
      'snake_case makes one endpoint speak both, and the same row read\n' +
      'through two routes come back with two sets of names.',
  );
  process.exit(1);
}

console.log(`✓ ${keys} wire keys across ${files.length} v1 handlers, all camelCase`);
