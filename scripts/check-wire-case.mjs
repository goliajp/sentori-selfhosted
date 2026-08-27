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
const HANDLERS = join(root, 'self-hosted/server/src/handlers');
const ROOT = join(HANDLERS, 'sdk');
const MOD = join(HANDLERS, 'mod.rs');

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

// Everything under `sdk/` serves v1 — but not everything that serves
// v1 lives under `sdk/`. Reading only that directory is how this
// checker reported "all camelCase" for years while
// `/v1/releases/{release}/artifacts`, which lives one level up,
// answered content_hash / size_bytes / debug_id / first_seen /
// content_changed. So take the routes as the source of truth: any
// handler reachable from a `/v1/` route is in scope, wherever it sits.
function v1HandlerFiles() {
  const src = readFileSync(MOD, 'utf8');
  const out = new Set();
  const unresolved = [];
  // One `.route(` call at a time, so a path found inside it belongs
  // to that route's own URL rather than a neighbour's.
  for (const call of src.split('.route(').slice(1)) {
    // Trailing chain calls (`.layer(…)`, `.with_state(…)`) after the
    // last route in a chain are not part of it. `indexOf` answering
    // -1 must mean "all of it", not `slice(0, -1)`.
    const cut = call.indexOf('\n        .');
    const block = cut === -1 ? call : call.slice(0, cut);
    if (!/"\/v1\//.test(block)) continue;
    for (const [, path] of block.matchAll(/\b((?:[a-z][a-z0-9_]*::)+)[a-z_]+\s*\)/g)) {
      const segs = path.split('::').filter(Boolean);
      // `a::b::c` is handlers/a/b/c.rs or handlers/a/b.rs — try the
      // longest first, since a directory and a module can share a name.
      let found = null;
      for (let n = segs.length; n > 0 && !found; n--) {
        const cand = join(HANDLERS, ...segs.slice(0, n)) + '.rs';
        try {
          if (statSync(cand).isFile()) found = cand;
        } catch {
          /* try a shorter path */
        }
      }
      if (found) out.add(found);
      else unresolved.push(segs.join('::'));
    }
  }
  return { files: [...out], unresolved };
}

const { files: routed, unresolved } = v1HandlerFiles();
if (unresolved.length) {
  // Silently skipping a handler is the failure this rewrite exists to
  // stop repeating, so an unreadable route is loud.
  console.error(`✗ could not locate the handler for: ${unresolved.join(', ')}`);
  process.exit(1);
}
const files = [...new Set([...walk(ROOT), ...routed])];
if (files.length === 0) {
  console.error('✗ no handlers found — this checker read nothing and passed');
  process.exit(1);
}
const outsideSdk = routed.filter((f) => !f.startsWith(ROOT));
if (outsideSdk.length === 0) {
  console.error('✗ the route scan found nothing outside sdk/ — it is not reading the router');
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
