// Every `INSERT … (cols) VALUES (…)` must balance.
//
// Two of them did not, and both were load-bearing:
//
//   device_tokens — 5 columns, 6 placeholders. Every device
//   registration any SDK ever attempted returned 500.
//   push_sends    — 9 columns, 10 values (`\'queued\'` shifted the
//   rest). Every send returned 500.
//
// Postgres refuses such a statement at prepare time, so the failure
// is total and silent from the outside: an endpoint that always
// 500s looks exactly like a feature nobody uses. The whole push
// subsystem read as "shipped but unadopted" for a year on the
// strength of two miscounted parentheses.
//
// Nothing else looks. `sqlx::query` takes a string; rustc never
// counts it, and a test only catches it if something calls that
// exact endpoint.
//
//   node scripts/check-sql-inserts.mjs

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';

const ROOTS = ['self-hosted/server/src', 'core/crates'];

function walk(dir, out = []) {
  let entries;
  try {
    entries = readdirSync(dir);
  } catch {
    return out;
  }
  for (const e of entries) {
    if (e === 'target') continue;
    const p = join(dir, e);
    if (statSync(p).isDirectory()) walk(p, out);
    else if (p.endsWith('.rs')) out.push(p);
  }
  return out;
}

/** Split on commas that are not inside brackets or quotes. */
function fields(s) {
  const out = [];
  let depth = 0;
  let cur = '';
  for (const c of s) {
    if (c === '(' || c === '[') depth += 1;
    else if (c === ')' || c === ']') depth -= 1;
    if (c === ',' && depth === 0) {
      out.push(cur);
      cur = '';
    } else cur += c;
  }
  out.push(cur);
  return out.map((x) => x.trim()).filter((x) => x.length > 0);
}

const files = ROOTS.flatMap((r) => walk(r));
if (files.length === 0) {
  console.error(`✗ no .rs under ${ROOTS.join(', ')} — this checker read nothing.`);
  process.exit(1);
}

/** The text inside the parenthesis group starting at `i`, and the
 *  index just past it. Counting brackets rather than scanning to the
 *  first `)` — `gen_random_uuid()` inside a VALUES list is common,
 *  and a naive scan reports every such statement as unbalanced. A
 *  checker that cries wolf gets switched off. */
function group(s, i) {
  let depth = 0;
  for (let j = i; j < s.length; j += 1) {
    if (s[j] === '(') depth += 1;
    else if (s[j] === ')') {
      depth -= 1;
      if (depth === 0) return [s.slice(i + 1, j), j + 1];
    }
  }
  return [null, s.length];
}

const problems = [];
let checked = 0;

for (const f of files) {
  // Rust string continuations (`\` at EOL) are not part of the SQL.
  const sql = readFileSync(f, 'utf8').replace(/\\\s*\n\s*/g, ' ');
  for (const m of sql.matchAll(/INSERT INTO (\w+)\s*\(/g)) {
    const open = m.index + m[0].length - 1;
    const [cols, afterCols] = group(sql, open);
    if (cols === null) continue;
    const rest = sql.slice(afterCols, afterCols + 600);
    const vm = /VALUES\s*\(/.exec(rest);
    // `INSERT … SELECT` has no VALUES list and nothing to balance.
    if (!vm) continue;
    const [vals] = group(rest, vm.index + vm[0].length - 1);
    if (vals === null) continue;
    checked += 1;
    const nc = fields(cols).length;
    const nv = fields(vals).length;
    if (nc !== nv) {
      problems.push(`${f}: INSERT INTO ${m[1]} — ${nc} columns, ${nv} values`);
    }
  }
}

// ── the same fault on the way out ────────────────────────────────
//
// `row.get("name")` panics with `ColumnNotFound` when the SELECT did
// not ask for that column. A panic inside a handler drops the
// connection, so the browser sees no status code at all — the client
// gets `curl: (52) Empty reply from server`, which names the shape of
// the failure and never the cause.
//
// `admin/releases.rs` read `usable` off a `releases` row and selected
// four other things. Every call to the dashboard's releases list
// panicked from server 2.15.0 to 2.21.1. The one CI job that touches
// that route had path filters covering only the RN Android module, so
// it did not run for six server releases.
//
// Judged per function, and only for functions that build their own
// query — a function reading rows handed to it by a caller has no
// SELECT here to check against, and guessing would make this noisy
// enough to be ignored.

/** Function bodies, keyed by name. Rough but sufficient: item-level
 *  `fn` in this codebase always starts at column 0 or one indent. */
function functions(src) {
  const out = [];
  const re = /^[ \t]*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+(\w+)/gm;
  const starts = [...src.matchAll(re)];
  for (let i = 0; i < starts.length; i += 1) {
    const from = starts[i].index;
    const to = i + 1 < starts.length ? starts[i + 1].index : src.length;
    out.push({ name: starts[i][1], body: src.slice(from, to) });
  }
  return out;
}

let readsChecked = 0;

for (const f of files) {
  const src = readFileSync(f, 'utf8').replace(/\\\s*\n\s*/g, ' ');
  for (const fn of functions(src)) {
    // Every string literal in the function that looks like a query.
    const queries = [...fn.body.matchAll(/"([^"\\]*(?:\\.[^"\\]*)*)"/g)]
      .map((m) => m[1])
      .filter((s) => /\bSELECT\b|\bRETURNING\b/i.test(s));
    if (queries.length === 0) continue;
    const text = queries.join(' ');
    // `SELECT *` brings columns this script cannot enumerate.
    if (/SELECT\s+\*|\.\*/i.test(text)) continue;
    // `serde_json::Value::get` is spelled identically to sqlx's
    // `Row::get`, and this file has plenty of both — reading a
    // credential blob's `"teamId"` is not a column read. Only the two
    // spellings that cannot be anything but sqlx count:
    //
    //   r.get::<Option<bool>, _>("usable")     turbofished
    //   let id: Uuid = row.get("id");          typed binding
    //
    // Inferred untyped reads (`rfc3339(r.get("created_at"))`) are not
    // covered. A gate over a subset with no false alarms is worth more
    // than a complete one that gets switched off — which is the same
    // reason `group()` above counts brackets.
    const reads = [
      // Bounded by the `(`, not by the first `>`: a turbofish never
      // contains a paren, but it very often contains a nested angle
      // bracket. `[^>]*` stopped inside `Option<bool>` and `Vec<String>`
      // — so the first version of this check silently skipped exactly
      // the reads it was written for, and passed on the injected bug.
      ...fn.body.matchAll(/\.(?:try_)?get::<[^()]*>\(\s*"([^"]+)"\s*\)/g),
      ...fn.body.matchAll(/\blet\s+\w+\s*:[^=;]+=\s*\w+\.(?:try_)?get\(\s*"([^"]+)"\s*\)/g),
    ];
    for (const g of reads) {
      readsChecked += 1;
      // Word boundary: `key` must not be satisfied by `user_key`.
      if (!new RegExp(`\\b${g[1].replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}\\b`).test(text)) {
        problems.push(`${f}: fn ${fn.name} reads "${g[1]}", which its query never selects`);
      }
    }
  }
}

// ── the shadow table ──────────────────────────────────────────────
//
// `push_tokens` is the push-provider crate's own store, kept up to
// date by a dual write and read by nothing in the live path. Send
// targeting, quarantine and the worker all use `device_tokens`.
//
// `DELETE /v1/push/tokens/{handle}` deleted from `push_tokens`,
// answered 202 `{"status":"revoked"}`, and left the device perfectly
// deliverable. A host called `unregister`, was told it worked, and
// the next send went out. Nothing catches that: both tables exist,
// both statements are valid, and the wrong one succeeds.
//
// So: the server's own SQL names `device_tokens`. The crate may keep
// its table; a handler that reaches for it is reaching for a copy.
const SHADOW = 'push_tokens';
let shadowHits = 0;
for (const file of walk('self-hosted/server/src')) {
  if (!file.endsWith('.rs')) continue;
  const src = readFileSync(file, 'utf8');
  for (const line of src.split('\n')) {
    const code = line.trim();
    if (code.startsWith('//')) continue;
    // Only SQL. `state.push_tokens` is the crate's store by another
    // name and is not what this is about.
    if (!/\b(FROM|INTO|UPDATE|JOIN)\s+push_tokens\b/i.test(code)) continue;
    shadowHits += 1;
    problems.push(
      `${file}: SQL naming '${SHADOW}'. The live path reads 'device_tokens' — ` +
        'targeting filters it, quarantine writes it, the worker joins it. A ' +
        'statement against the other table succeeds and changes nothing anyone ' +
        `reads: \`${code.slice(0, 70)}\``,
    );
  }
}

if (problems.length === 0) {
  console.log(
    `✓ ${checked} INSERT statements balance; ${readsChecked} column reads are ` +
      `selected; no handler reaches for '${SHADOW}' (${shadowHits} found)`,
  );
  process.exit(0);
}
for (const p of problems) console.error(`✗ ${p}`);
console.error(
  '\nPostgres refuses an unbalanced INSERT at prepare time, and panics a\n' +
    'handler that reads an unselected column. Either way the endpoint never\n' +
    'answers — which from the outside is indistinguishable from a feature\n' +
    'nobody uses.',
);
process.exit(1);
