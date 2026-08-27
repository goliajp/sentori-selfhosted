// Every SENTORI_* variable the docs name must be one the code reads,
// and the ones a self-hoster is told to set must reach the container.
//
// troubleshooting.md told operators to raise
// `SENTORI_RATE_LIMIT_PER_MIN`. No such variable exists — the real one
// is `SENTORI_RATELIMIT_PER_TOKEN_RPS`, with a different unit and a
// different order of magnitude. And every rate-limit knob
// protocol.md documents was absent from the compose file, so a
// self-hoster could not set any of them whatever the reference said.
//
// Two failures, one shape: a name in a document that nothing on the
// other side answers to.
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';

const ROOT = new URL('..', import.meta.url).pathname;
const DOC_SKIP = /^docs\/(archive|design|plans|roadmap|dogfood|performance|perf-baselines)\//;
const COMPOSE = 'self-hosted/docker/docker-compose.yml';

// ── names the code actually reads ──────────────────────────────────
const read = new Set();
const walkRs = (d) => {
  for (const e of readdirSync(join(ROOT, d))) {
    const rel = `${d}/${e}`;
    if (rel.includes('/target/')) continue;
    if (statSync(join(ROOT, rel)).isDirectory()) walkRs(rel);
    else if (e.endsWith('.rs')) {
      for (const m of readFileSync(join(ROOT, rel), 'utf8')
        .matchAll(/"(SENTORI_[A-Z0-9_]+)"/g))
        read.add(m[1]);
    }
  }
};
walkRs('self-hosted/server/src');
walkRs('core');

// The CLI reads its own; a doc naming one of those is documenting a
// real variable even though no Rust file mentions it.
const walkTs = (d) => {
  for (const e of readdirSync(join(ROOT, d))) {
    const rel = `${d}/${e}`;
    if (/node_modules|\/lib\/|\/dist\//.test(rel)) continue;
    if (statSync(join(ROOT, rel)).isDirectory()) walkTs(rel);
    else if (/\.(ts|tsx|mjs)$/.test(e)) {
      for (const m of readFileSync(join(ROOT, rel), 'utf8')
        .matchAll(/process\.env\.(SENTORI_[A-Z0-9_]+)|process\.env\[['"`](SENTORI_[A-Z0-9_]+)/g))
        read.add(m[1] ?? m[2]);
    }
  }
};
walkTs('sdk/cli/src');
walkTs('scripts');

if (read.size < 5) {
  console.error(`✗ found ${read.size} SENTORI_* reads in the source. Broken checker.`);
  process.exit(1);
}

// Variables consumed by shell/CI rather than the server binary.
const NON_SERVER = new Set([
  'SENTORI_VERSION', 'SENTORI_PORT', 'SENTORI_PG_IMAGE',
  'SENTORI_E2E_KEEP', 'SENTORI_SELFHOSTED_MIRROR_TOKEN',
  'SENTORI_SELFHOSTED_MIRROR_REPO', 'SENTORI_INGEST_URL', 'SENTORI_TOKEN',
  'SENTORI_BASE', 'SENTORI_API_TOKEN', 'SENTORI_ADMIN_TOKEN',
  'SENTORI_PROJECT_ID', 'SENTORI_BIND',
  // NOT exempt: SENTORI_RATE_LIMIT_PER_MIN. It was on this list in the
  // first draft, which is how a checker written to catch that exact
  // name ended up unable to. An exemption list is for variables the
  // server legitimately does not read — not for making a red gate
  // green.
]);

// ── names the docs claim ───────────────────────────────────────────
const docs = [];
const walkMd = (d) => {
  for (const e of readdirSync(join(ROOT, d))) {
    const rel = `${d}/${e}`;
    if (DOC_SKIP.test(rel)) continue;
    if (statSync(join(ROOT, rel)).isDirectory()) walkMd(rel);
    else if (e.endsWith('.md')) docs.push(rel);
  }
};
walkMd('docs');

const unknown = [];
for (const rel of docs) {
  const text = readFileSync(join(ROOT, rel), 'utf8');
  text.split('\n').forEach((line, i) => {
    for (const m of line.matchAll(/`(SENTORI_[A-Z0-9_]+)`/g)) {
      const name = m[1];
      // `X_FILE` is a Docker-secret variant resolved generically in
      // env_config.rs — `env(key + "_FILE")` — so it never appears as
      // a literal. The first version of this checker reported two real
      // variables as fictional for exactly that reason.
      const base = name.endsWith('_FILE') ? name.slice(0, -5) : null;
      if (read.has(name) || NON_SERVER.has(name)) continue;
      if (base && (read.has(base) || NON_SERVER.has(base))) continue;
      // A doc may name a variable in order to say it is gone.
      if (/no such|does not exist|removed|was renamed|until /i.test(line)) continue;
      unknown.push({ rel, line: i + 1, name });
    }
  });
}

// ── the compose file must offer what the docs tell people to set ───
const compose = readFileSync(join(ROOT, COMPOSE), 'utf8');
const MUST_REACH = [
  'SENTORI_RATELIMIT_PER_TOKEN_RPS',
  'SENTORI_RATELIMIT_WINDOW_SEC',
  'SENTORI_RATELIMIT_DISABLED',
];
const missing = MUST_REACH.filter((v) => !compose.includes(v));

if (unknown.length || missing.length) {
  for (const u of unknown)
    console.error(`✗ ${u.rel}:${u.line} names \`${u.name}\`, which nothing reads`);
  for (const v of missing)
    console.error(`✗ ${COMPOSE} does not pass \`${v}\` through, so a self-hoster cannot set it`);
  process.exit(1);
}
console.log(
  `✓ ${docs.length} docs name only SENTORI_* variables the code reads; ` +
    `compose passes the tunable ones`,
);
