// Two things go wrong with a message catalogue, and neither is visible
// in a diff.
//
// 1. Locales drift apart. `Messages` being derived from en.ts makes a
//    *missing* key a compile error, but nothing catches a key that only
//    exists in zh.
// 2. A key nobody references. The instinct is to call that dead copy,
//    but every time it has meant the opposite: the string was added,
//    the call site was never wired up, and the screen is still English.
//    `alerts.newRule`, `issues.empty` and `action.signOut` were all
//    found this way — each one a button still reading English on an
//    otherwise translated page.
//
// Exit 1 on either. Run from webapp/: `node devtools/check-i18n.mjs`.
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

const SRC = 'src';
const LOCALES = ['en', 'zh', 'ja'];

const keysOf = loc => {
  const text = readFileSync(`${SRC}/i18n/${loc}.ts`, 'utf8');
  return new Set([...text.matchAll(/^ {2}'([\w.]+)':/gm)].map(m => m[1]));
};

const walk = dir =>
  readdirSync(dir, { withFileTypes: true }).flatMap(e => {
    const p = join(dir, e.name);
    if (e.isDirectory()) return walk(p);
    return /\.tsx?$/.test(p) && !p.includes('/i18n/') ? [p] : [];
  });

const catalogues = Object.fromEntries(LOCALES.map(l => [l, keysOf(l)]));
const problems = [];

// 1 — same keys everywhere.
const en = catalogues.en;
for (const loc of LOCALES.slice(1)) {
  for (const k of catalogues[loc]) {
    if (!en.has(k)) problems.push(`${loc}.ts has '${k}', en.ts does not`);
  }
  for (const k of en) {
    if (!catalogues[loc].has(k)) problems.push(`${loc}.ts is missing '${k}'`);
  }
}

// 2 — every key reaches a call site.
const used = new Set();
const prefixes = new Set();
for (const f of walk(SRC)) {
  const text = readFileSync(f, 'utf8');
  for (const m of text.matchAll(/t\(\s*'([\w.]+)'/g)) used.add(m[1]);
  // Chosen at the call site: t(n === 1 ? 'crash.step' : 'crash.steps')
  for (const m of text.matchAll(/t\([^)]*\?\s*'([\w.]+)'\s*:\s*'([\w.]+)'/g)) {
    used.add(m[1]);
    used.add(m[2]);
  }
  // Keys carried in a table rather than called directly:
  //   const KIND_KEYS = { quota: 'notifications.kindQuota' }
  for (const m of text.matchAll(/:\s*'([\w.]+\.[\w.]+)'/g)) used.add(m[1]);
  // Built at the call site: t(`status.${row.status}`)
  for (const m of text.matchAll(/t\(`([\w.]+)\.\$\{/g)) prefixes.add(m[1]);
}
for (const k of en) {
  if (used.has(k)) continue;
  if ([...prefixes].some(p => k.startsWith(`${p}.`))) continue;
  problems.push(
    `'${k}' is in the catalogue but nothing uses it — usually a screen ` +
      `that is still hard-coded English`,
  );
}

if (problems.length) {
  for (const p of problems) console.error(`i18n: ${p}`);
  console.error(`\n${problems.length} problem(s)`);
  process.exit(1);
}
console.log(`✓ ${en.size} keys, ${LOCALES.length} locales, all referenced`);
