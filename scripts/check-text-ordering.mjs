// An order the operator's docker image decides is not an order.
//
// `ORDER BY name` sorts by the *database's* collation. Ours is not one
// thing: `docker-compose.yml` ships postgres:18-alpine, which is musl —
// it declares `en_US.utf8` and behaves as C — while a customer pointing
// Sentori at their own glibc Postgres gets real en_US.utf8. Measured on
// 2026-08-19 with `tmp/spg-repro/divergence`: eight of eighteen probes
// disagree between those two, including which rows a text range returns.
//
// So the same Sentori listed the same projects in two different orders
// depending on an image tag, and nothing said so.
//
// Two forms are allowed and nothing else:
//
//   * `ORDER BY col COLLATE "C"` — byte order, identical everywhere.
//     For anything a machine reads or an operator diffs.
//   * no server-side order at all — the console sorts in the viewer's
//     language with `Intl.Collator`, which is both stable across
//     deployments and actually right for the reader. This console is
//     trilingual; a database never knows which language is on screen.
//
// Scoped to text columns. `ORDER BY created_at` is not collation's
// business.

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const ROOTS = [join(root, 'self-hosted/server/src'), join(root, 'core/crates')];

/** Column names that hold text a human supplied or a locale can reorder. */
const TEXT = [
  'name', 'email', 'label', 'title', 'culprit', 'message', 'release',
  'environment', 'platform', 'reason', 'topic', 'val', 'slug',
];

function walk(dir, out = []) {
  for (const e of readdirSync(dir)) {
    const p = join(dir, e);
    if (p.includes('/target/')) continue;
    if (statSync(p).isDirectory()) walk(p, out);
    else if (p.endsWith('.rs')) out.push(p);
  }
  return out;
}

const files = ROOTS.flatMap((r) => walk(r));
if (files.length === 0) {
  console.error('✗ no Rust sources found — this checker read nothing and passed');
  process.exit(1);
}

const findings = [];
let ordered = 0;

for (const f of files) {
  const src = readFileSync(f, 'utf8');
  // SQL literals only — `ORDER BY` in a comment is prose.
  //
  // `(?:[^"\\]|\\.)*` rather than `[^"]*`: a Rust literal containing
  // `COLLATE \"C\"` has escaped quotes in it, and a naive character
  // class ends the match at the first one. The first version of this
  // checker did exactly that and reported the three statements that
  // carry the fix as violations of it.
  for (const m of src.matchAll(/"((?:SELECT|WITH)(?:[^"\\]|\\.){10,})"/gs)) {
    // Collapse only Rust's line continuation — a backslash at end of
    // line. Replacing every backslash turns `COLLATE \"C\"` into
    // `COLLATE "C "`, which is what the first version did, and then
    // nothing matched.
    const stmt = m[1].replace(/\\\n\s*/g, ' ').replace(/\s+/g, ' ');
    // `[^"]` cannot be the terminator: the statements that carry the
    // fix contain quotes, so it stopped at the first `COLLATE \"`.
    for (const o of stmt.matchAll(/ORDER BY (.*?)(?:\bLIMIT\b|\bOFFSET\b|$)/gis)) {
      for (const term of o[1].split(',')) {
        const col = term.trim().replace(/^[a-z_]+\./, '').split(/\s+/)[0];
        if (!TEXT.includes(col.toLowerCase())) continue;
        ordered += 1;
        if (!/COLLATE\s+\\?"C\\?"/i.test(term)) {
          const line = src.slice(0, m.index).split('\n').length;
          findings.push(`${f.replace(`${root}/`, '')}:${line}  ORDER BY ${term.trim()}`);
        }
      }
    }
  }
}

// Finding nothing is a broken checker, not a clean tree.
//
// A regex change made this report "0 text-column orderings, all
// collation-independent" while it was matching no statements at all —
// and the run before it had found six. The zero is what exposed a
// comment accidentally written INSIDE a SQL string literal, which
// `cargo check` cannot see and Postgres would have rejected at
// runtime.
if (ordered < 4) {
  console.error(
    `✗ only ${ordered} text-column orderings found across ${files.length} files — ` +
      'this checker is broken, not the tree. It has found six before now.',
  );
  process.exit(1);
}

if (findings.length > 0) {
  console.error('✗ a text column is ordered by the database\'s collation:\n');
  for (const f of findings) console.error(`    ${f}`);
  console.error(
    '\nThat order depends on which postgres image the operator pulled —\n' +
      'ours ships alpine (musl, behaves as C), a customer\'s own is glibc.\n' +
      'Either add COLLATE "C" for a stable machine-readable order, or drop\n' +
      'the ORDER BY and sort in the console with Intl.Collator.',
  );
  process.exit(1);
}

console.log(`✓ ${ordered} text-column orderings, all collation-independent`);
