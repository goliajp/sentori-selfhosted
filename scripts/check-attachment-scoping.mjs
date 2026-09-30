// Every read of `event_attachments` must be scoped by project.
//
// An attachment carries the *uploader's* project, and the upload
// handler deliberately does not require the event to exist — evidence
// legitimately arrives before the crash it belongs to. That makes
// `WHERE event_id = $1` alone a cross-tenant write channel: any
// project's ingest token could plant a row that renders inside
// another project's crash detail as its own evidence.
//
// Three reads had it. This is the rule that keeps the fourth from
// being written.
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';

const ROOT = new URL('../self-hosted/server/src/', import.meta.url).pathname;
// The archive worker sweeps every project by design; it deletes by a
// pre-scoped event id list and never renders anything to a caller.
const EXEMPT = new Set(['archive_worker.rs']);

const files = [];
const walk = (d) => {
  for (const e of readdirSync(join(ROOT, d))) {
    const rel = d ? `${d}/${e}` : e;
    if (statSync(join(ROOT, rel)).isDirectory()) walk(rel);
    else if (e.endsWith('.rs')) files.push(rel);
  }
};
walk('');

const bad = [];
let checked = 0;
for (const rel of files) {
  if (EXEMPT.has(rel.split('/').pop())) continue;
  const src = readFileSync(join(ROOT, rel), 'utf8');
  // Each SQL string literal that reads the table.
  for (const m of src.matchAll(/"((?:[^"\\]|\\[\s\S])*event_attachments(?:[^"\\]|\\[\s\S])*)"/g)) {
    const sql = m[1].replace(/\\\s+/g, ' ').replace(/\s+/g, ' ');
    if (!/SELECT/i.test(sql)) continue;
    checked++;
    if (!/project_id/i.test(sql)) {
      bad.push({ rel, sql: sql.slice(0, 150) });
    }
  }
}

if (checked === 0) {
  console.error(
    `✗ found no SELECT against event_attachments. This checker is broken, ` +
      `not the tree.`,
  );
  process.exit(1);
}

if (bad.length > 0) {
  console.error(`✗ ${bad.length} read(s) of event_attachments with no project scope:`);
  for (const b of bad) console.error(`    ${b.rel}\n      ${b.sql}`);
  console.error(
    `  An attachment is stamped with the uploader's project, and the ` +
      `upload path does not require the event to exist. Without the ` +
      `predicate, one project can write into another's evidence.`,
  );
  process.exit(1);
}
console.log(`✓ ${checked} reads of event_attachments, all project-scoped`);
