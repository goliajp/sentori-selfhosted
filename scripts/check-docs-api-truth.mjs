// Documentation may only name APIs and packages that exist.
//
// The user-facing docs described a product that is partly gone. Guides
// for React, Next.js, Remix and Vite pointed at `@goliapkg/sentori-react`
// — a package that is not on npm — and at `SentoriProvider`, which is
// not exported. A recipe taught `sentori.startSpan`, troubleshooting
// taught `sentori.startSession`, and neither symbol exists anywhere in
// the SDK. An external issue-tracker page described four adapters with
// no endpoint, no table and no migration behind them.
//
// None of that is cosmetic. An AI agent reading these writes an
// integration against a wire format this server answers with
// `400 invalid_payload`, and a human reading them installs a package
// that 404s. Documentation that names things which do not exist is
// worse than no documentation: it is confidently wrong, and both
// readers act on it.
//
// The rule: every `sentori.<name>(` in a doc must be an export of the
// React Native SDK, and every `@goliapkg/<pkg>` must be a package this
// repository actually builds.
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';

const ROOT = new URL('..', import.meta.url).pathname;
const DOC_DIRS = ['docs'];
// `archive/` is where pages that name a vanished API are allowed to
// live — see docs/archive/README.md. The other trees are internal
// notes that were never a contract with anyone.
const SKIP = /^docs\/(archive|design|plans|roadmap|dogfood|performance|perf-baselines|runbook)\//;

// ── what actually exists ────────────────────────────────────────────
const rn = (p) => readFileSync(join(ROOT, 'sdk/react-native/src', p), 'utf8');
const exported = new Set();
for (const m of rn('index.ts').matchAll(
  /export\s+(?:const|function|class)\s+(\w+)|export\s+\{([^}]*)\}/g,
)) {
  if (m[1]) exported.add(m[1]);
  for (const n of (m[2] ?? '').split(','))
    exported.add(n.replace(/^\s*type\s+/, '').split(/\s+as\s+/).pop().trim());
}
for (const m of rn('verbs.ts').matchAll(/export const (\w+)/g)) exported.add(m[1]);

// `sentori` is an object literal, and its keys are the API almost
// every doc actually writes — `sentori.user(...)`, `sentori.context(...)`.
// Reading only `export const` missed them, and the first run of this
// checker reported two correct pages as wrong. A gate that is wrong in
// this direction is worse than none: it argues for breaking the docs.
{
  const idx = rn('index.ts');
  const start = idx.indexOf('export const sentori = {');
  if (start !== -1) {
    let i = idx.indexOf('{', start), depth = 0, end = i;
    for (; i < idx.length; i++) {
      if (idx[i] === '{') depth++;
      else if (idx[i] === '}' && --depth === 0) { end = i; break; }
    }
    const lit = idx.slice(start, end);
    // top-level keys only: `  name:` at exactly two spaces of indent
    for (const m of lit.matchAll(/^ {2}(\w+):/gm)) exported.add(m[1]);
  }
}
exported.delete('');

const packages = new Set();
const sdkRoot = join(ROOT, 'sdk');
for (const e of readdirSync(sdkRoot)) {
  const pj = join(sdkRoot, e, 'package.json');
  try {
    packages.add(JSON.parse(readFileSync(pj, 'utf8')).name);
  } catch { /* not a package */ }
}

if (exported.size < 5 || packages.size < 2) {
  console.error(
    `✗ resolved ${exported.size} SDK exports and ${packages.size} packages. ` +
      `This checker is broken, not the tree — it cannot judge on an empty list.`,
  );
  process.exit(1);
}

// ── what the docs claim ─────────────────────────────────────────────
const files = [];
const walk = (d) => {
  for (const e of readdirSync(join(ROOT, d))) {
    const rel = `${d}/${e}`;
    if (statSync(join(ROOT, rel)).isDirectory()) walk(rel);
    else if (e.endsWith('.md') && !SKIP.test(rel)) files.push(rel);
  }
};
for (const d of DOC_DIRS) walk(d);

const bad = [];
for (const rel of files) {
  const text = readFileSync(join(ROOT, rel), 'utf8');
  const lines = text.split('\n');
  lines.forEach((line, i) => {
    // A sentence explaining that something is gone runs across lines,
    // so the disowning window is this line and its neighbours — the
    // first version tested one line and flagged the very paragraph
    // that exists to say the package does not.
    const near = lines.slice(Math.max(0, i - 1), i + 2).join(' ');
    const disowned =
      /never existed|no longer|left this repo|not on npm|deleted|removed|do not integrate|there were guides/i.test(
        near,
      );
    // `sentori.foo(` — a call, not `sentori.golia.jp`
    for (const m of line.matchAll(/\bsentori\.(\w+)\s*\(/g)) {
      if (!exported.has(m[1]) && !disowned)
        bad.push({ rel, line: i + 1, what: `sentori.${m[1]}()`, kind: 'export' });
    }
    for (const m of line.matchAll(/@goliapkg\/([a-z0-9-]+)/g)) {
      if (!packages.has(`@goliapkg/${m[1]}`) && !disowned)
        bad.push({ rel, line: i + 1, what: `@goliapkg/${m[1]}`, kind: 'package' });
    }
  });
}

if (bad.length > 0) {
  const byFile = new Map();
  for (const b of bad) {
    if (!byFile.has(b.rel)) byFile.set(b.rel, []);
    byFile.get(b.rel).push(b);
  }
  console.error(`✗ docs name ${bad.length} thing(s) that do not exist:\n`);
  for (const [rel, list] of byFile) {
    console.error(`  ${rel}`);
    for (const b of list.slice(0, 4))
      console.error(`    :${b.line}  ${b.what}  (no such ${b.kind})`);
    if (list.length > 4) console.error(`    … and ${list.length - 4} more`);
  }
  console.error(
    `\n  SDK exports: ${[...exported].sort().join(', ')}` +
      `\n  packages:    ${[...packages].sort().join(', ')}`,
  );
  process.exit(1);
}

console.log(
  `✓ ${files.length} docs name only real APIs ` +
    `(${exported.size} exports, ${packages.size} packages)`,
);
