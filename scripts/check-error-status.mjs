// A response carrying an `error` must not arrive as 200.
//
// Four handlers returned a bare `Json<Value>` — no status alongside it,
// so axum sends 200 — and put an `error` key in the body anyway. The
// worst was `GET /v1/push/users/{key}/preferences`, which answered
// `200 {"preferences": [], "error": "internal"}` when its query failed.
// A caller reading `preferences` sees "this person has opted out of
// nothing" where the truth is "we do not know", and then sends to
// somebody who may have opted out.
//
// Two others answered `200 {"sends": []}` and `200 {"credentials": []}`
// on an authorisation failure, where eleven sibling routes on the same
// project answer 403 — so "you may not look" and "there are none" were
// the same response, which is the exact question a setup screen asks.
//
// The rule: a handler whose return type is a bare `Json<...>` may not
// build a body with an `error` key. Carry the status.
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';

const ROOT = new URL('../self-hosted/server/src/handlers/', import.meta.url).pathname;

const files = [];
const walk = (d) => {
  for (const e of readdirSync(join(ROOT, d))) {
    const rel = d ? `${d}/${e}` : e;
    if (statSync(join(ROOT, rel)).isDirectory()) walk(rel);
    else if (e.endsWith('.rs')) files.push(rel);
  }
};
walk('');

let handlers = 0;
const bad = [];
for (const rel of files) {
  const src = readFileSync(join(ROOT, rel), 'utf8');
  const fns = [...src.matchAll(/pub async fn (\w+)\s*\(/g)];
  fns.forEach((m, i) => {
    const start = m.index;
    const end = i + 1 < fns.length ? fns[i + 1].index : src.length;
    const body = src.slice(start, end);
    const brace = body.indexOf('{');
    if (brace < 0) return;
    const sig = body.slice(0, brace);
    const ret = /->\s*([^{]+)/.exec(sig);
    if (!ret) return;
    handlers++;
    const rt = ret[1].trim();
    // A bare `Json<...>` return type means axum sends 200, always.
    if (!/^Json<[^>]*>$/.test(rt)) return;
    if (/"error"\s*:/.test(body)) {
      const codes = [...body.matchAll(/"error"\s*:\s*"([a-z_ ]+)"/g)].map((x) => x[1]);
      bad.push({ rel, name: m[1], codes: [...new Set(codes)].slice(0, 3) });
    }
  });
}

if (handlers < 20) {
  console.error(`✗ parsed ${handlers} handlers. This checker is broken, not the tree.`);
  process.exit(1);
}

if (bad.length > 0) {
  console.error(`✗ ${bad.length} handler(s) answer 200 with an error in the body:`);
  for (const b of bad)
    console.error(`    ${b.rel}::${b.name}  → ${b.codes.join(', ') || 'error'}`);
  console.error(
    '  Return (StatusCode, Json<...>). A client cannot branch on a status ' +
      'that is 200 whatever happened.',
  );
  process.exit(1);
}
console.log(`✓ ${handlers} handlers: no 200 carries an error body`);
