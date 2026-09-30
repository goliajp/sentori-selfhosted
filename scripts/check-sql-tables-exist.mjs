// Every table our SQL names must exist in the migrations.
//
// Three metrics in `handlers/metrics_prom.rs` were wrong for the whole
// life of the v1 schema and all three read `0`:
//
//   FROM alert_rules      the table was removed by the v1 rewrite
//   FROM sessions         it has been `auth_sessions` since 0001
//   status = 'unresolved' a value the column's CHECK forbids
//
// The first two are this checker's job. Nothing raised, nobody looked,
// and a Prometheus gauge reading zero is what a healthy gauge reads
// most of the time — so the endpoint had been answering confidently
// and wrongly to whoever scraped it.
//
// The third is not: that query succeeds and matches nothing. It is
// here in the comment because it is the reminder that this gate closes
// one of the two ways a statement can be quietly meaningless, and that
// the other one needs the column's own constraints read.
//
// Scope: table names in FROM / JOIN / INSERT INTO / UPDATE / DELETE
// FROM, inside SQL string literals in Rust. Not the CLI's bare string
// lists of table names — those are data, not SQL, and 43 of their
// entries name tables the v1 rewrite removed. That is a separate fix
// with its own verification, because `commands::TABLES` also drives
// `restore` and its ORDER is load-bearing.

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const MIGRATIONS = join(root, 'core/migrations');
const ROOTS = [join(root, 'self-hosted'), join(root, 'core')];

/// Words that follow one of the keywords below without being a table.
///
/// `extract(epoch FROM now())` is not a scan of `now`; `DO UPDATE SET`
/// is not an update of `set`; `JOIN LATERAL (` is not a join to
/// `lateral`; `FOR UPDATE SKIP LOCKED` is not an update of `skip`.
/// The first version of this checker reported all four and would have
/// been switched off within a day — a gate that cries wolf is removed,
/// and then the real thing goes past.
const KEYWORDS = new Set([
  'set', 'lateral', 'now', 'skip', 'only', 'select', 'values', 'nowait',
  'timestamp', 'timestamptz', 'interval', 'current_date', 'current_timestamp',
]);

/** Tables PostgreSQL provides, plus the ones a migrator creates itself. */
const NOT_OURS = new Set([
  '_sqlx_migrations',
  'pg_index', 'pg_class', 'pg_attribute', 'pg_constraint', 'pg_namespace',
  'pg_type', 'pg_proc', 'pg_database', 'pg_stat_activity', 'pg_settings',
  'information_schema', 'generate_series', 'unnest', 'jsonb_object_keys',
  'jsonb_array_elements', 'jsonb_each', 'jsonb_each_text', 'regexp_split_to_table',
]);

const schema = new Set();
for (const f of readdirSync(MIGRATIONS).filter((f) => f.endsWith('.sql'))) {
  const src = readFileSync(join(MIGRATIONS, f), 'utf8');
  for (const m of src.matchAll(/CREATE TABLE (?:IF NOT EXISTS )?(\w+)/gi)) {
    schema.add(m[1].toLowerCase());
  }
}
// A schema this checker cannot read is a checker that passes everything.
if (schema.size < 20) {
  console.error(
    `✗ read only ${schema.size} tables from core/migrations — ` +
      'this checker is broken, not the tree. It has read 26 before now.',
  );
  process.exit(1);
}

function walk(dir, out = []) {
  let entries;
  try { entries = readdirSync(dir); } catch { return out; }
  for (const e of entries) {
    const p = join(dir, e);
    if (p.includes('/target/') || p.includes('/node_modules/')) continue;
    if (statSync(p).isDirectory()) walk(p, out);
    else if (p.endsWith('.rs')) out.push(p);
  }
  return out;
}

const findings = [];
let checked = 0;

for (const file of ROOTS.flatMap((r) => walk(r))) {
  const src = readFileSync(file, 'utf8');
  // SQL literals only. `(?:[^"\\]|\\.)*` rather than `[^"]*`: a
  // statement containing `COLLATE \"C\"` has escaped quotes in it and a
  // naive class ends the match at the first one.
  for (const m of src.matchAll(
    /"((?:SELECT|INSERT|UPDATE|DELETE|WITH)\b(?:[^"\\]|\\.){15,})"/gs,
  )) {
    const stmt = m[1].replace(/\\\n\s*/g, ' ').replace(/\\"/g, '"').replace(/\s+/g, ' ');
    // A statement assembled with `{}` is not the statement that runs.
    if (/\{[a-z_]*\}/i.test(stmt)) continue;
    const line = src.slice(0, m.index).split('\n').length;
    for (const t of stmt.matchAll(
      /\b(?:FROM|JOIN|INSERT\s+INTO|UPDATE|DELETE\s+FROM)\s+(?:ONLY\s+)?([a-z_][a-z0-9_]*)/gi,
    )) {
      const name = t[1].toLowerCase();
      if (KEYWORDS.has(name) || NOT_OURS.has(name)) continue;
      // A CTE names itself; so does a derived table's alias.
      if (new RegExp(`\\b${name}\\s+AS\\s*\\(`, 'i').test(stmt)) continue;
      if (new RegExp(`\\)\\s+(?:AS\\s+)?${name}\\b`, 'i').test(stmt)) continue;
      checked += 1;
      if (!schema.has(name)) {
        findings.push(
          `${file.replace(`${root}/`, '')}:${line}  ${name}\n      ${stmt.slice(0, 110)}`,
        );
      }
    }
  }
}

// Finding nothing is a broken checker, not a clean tree.
if (checked < 100) {
  console.error(
    `✗ only ${checked} table references found across our SQL — ` +
      'this checker is broken, not the tree. It has found 260 before now.',
  );
  process.exit(1);
}

if (findings.length > 0) {
  console.error('✗ our SQL names a table that no migration creates:\n');
  for (const f of findings) console.error(`    ${f}\n`);
  console.error(
    'Either the migration is missing or the name is wrong. Both have\n' +
      'happened: `sessions` for `auth_sessions`, and `alert_rules` for a\n' +
      'table the v1 rewrite removed. Neither raised anything a person saw.',
  );
  process.exit(1);
}

console.log(`✓ ${checked} table references, every one of them in the schema`);
