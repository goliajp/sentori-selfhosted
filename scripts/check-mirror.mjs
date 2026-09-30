// The public mirror must ship what a self-hoster needs — the docs and
// the build — and none of what they must not see.
//
// `docs/` was excluded wholesale while `docs/README.md` told readers
// they could find the files "here or in the OSS mirror". They could
// not: the mirror is where somebody goes to run this, and it shipped
// the server with no instructions. Opening it back up is an allowlist,
// and an allowlist in rsync is easy to get wrong — the first attempt
// added the includes without a terminating `--exclude='/docs/**'`, so
// every internal note and the whole archive went public instead. That
// is the failure this guards: not "did we forget a file" but "did we
// publish the ones that were meant to stay in".
//
// It runs the real rsync arguments out of the workflow rather than a
// restatement of them. `--dry-run` asks rsync what it would copy instead
// of copying it: the answer is the same and a working tree holds
// gigabytes of local build output that the copy had to walk through.
// The answer is intersected with the tracked files, because the mirror
// is built from a clean checkout and nothing untracked can reach it.
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const ROOT = new URL('..', import.meta.url).pathname;
const WF = '.github/workflows/v0.2-oss-mirror.yml';

const MUST_SHIP = [
  'docs/README.md',
  'docs/getting-started.md',
  'docs/getting-started/react-native.md',
  'docs/protocol.md',
  'docs/errors.md',
  'docs/troubleshooting.md',
  'docs/self-hosting.md',
  // `docs/README.md` links scaling.md, and the mirror is where someone
  // reads that README while running the thing. It shipped the README
  // and not the page, so the link 404'd for every reader of the public
  // repo — the failure this file's header describes, in the other
  // direction.
  'docs/runbook/scaling.md',
  'docs/runbook/cli-auth.md',
  // Every stack the product supports, because the mirror is where a
  // self-hoster reads how to point something at the instance they
  // just brought up. React Native was the only one listed, from when
  // it was the only one there was; a reader on any other stack found
  // a getting-started page whose table linked to four pages and
  // shipped one.
  'docs/sdk-swift.md',
  'docs/sdk-kotlin.md',
  'docs/getting-started/web.md',
  'docs/getting-started/weapp.md',
];
// Cloning the mirror and running `docker compose up --build` is the
// product. Nothing checked that the tree it produces can do that: the
// allowlist could drop a manifest, a migration or the Dockerfile and
// every gate here would stay green while the public repo was unbuildable.
const MUST_BUILD = [
  'self-hosted/docker/Dockerfile',
  'self-hosted/docker/docker-compose.yml',
  'self-hosted/server/Cargo.toml',
  'self-hosted/server/Cargo.lock',
  'core/Cargo.toml',
  'core/Cargo.lock',
  'webapp/package.json',
  'webapp/bun.lock',
  'LICENSE-APACHE',
  'LICENSE-MIT',
  'README.md',
];

// Trees that have held internal material. They are gone from the repo,
// so this is a tripwire rather than a filter: bring one back and the
// mirror must not carry it.
const MUST_NOT = /^docs\/(roadmap|design|plans|performance|perf-baselines|dogfood|infrastructure|archive|legal)\//;

const wf = readFileSync(join(ROOT, WF), 'utf8');
const m = wf.match(/rsync -a --delete \\\n([\s\S]*?)\n\s+\.\/ \/tmp\/mirror\//);
if (!m) {
  console.error(`✗ could not find the rsync invocation in ${WF}. This ` +
    `checker is broken, not the tree.`);
  process.exit(1);
}
const args = m[1]
  .split('\n')
  .map((l) => l.trim().replace(/\\$/, '').trim())
  .filter((l) => l.startsWith('--include') || l.startsWith('--exclude'))
  .map((l) => l.replace("='", '=').replace(/'$/, ''));

if (args.length < 10) {
  console.error(`✗ parsed ${args.length} rsync filters. Broken checker.`);
  process.exit(1);
}

// `--itemize-changes` with `--dry-run` prints one line per path rsync
// would transfer; `%n` makes that line the path itself. Directories come
// through with a trailing slash and are dropped.
const listed = execFileSync(
  'rsync',
  ['-a', '--delete', '--dry-run', '--out-format=%n', ...args, './', '/tmp/sentori-mirror-dryrun/'],
  { cwd: ROOT, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 },
)
  .split('\n')
  .map((l) => l.trim())
  .filter((l) => l && !l.endsWith('/') && l !== './');

// The mirror is built from a clean checkout, so only tracked files can
// reach it. A developer's tree also holds build output, and asking
// whether *that* would be published is a different and always-true
// question.
const tracked = new Set(
  execFileSync('git', ['ls-files'], { cwd: ROOT, encoding: 'utf8' }).split('\n').filter(Boolean),
);
const shipped = listed.filter((f) => tracked.has(f));

if (shipped.length < 50) {
  console.error(`\u2717 the dry run listed ${shipped.length} tracked files. ` +
    `Broken checker, not a broken tree.`);
  process.exit(1);
}

const docs = shipped.filter((f) => f.startsWith('docs/'));
const missingDocs = MUST_SHIP.filter((f) => !shipped.includes(f));

// And the other direction: a page in `docs/` that the allowlist does
// not name. The list above says which pages must ship; nothing said
// that every page must, so `docs/dashboard.md` was written, linked
// from the index, rendered on the site, and left out of the public
// repository — where the same index link then 404s. The allowlist is
// per-file on purpose, so this is the check that keeps adding a page
// and publishing it one action.
const tracked_docs = [...tracked].filter((f) => /^docs\/.*\.md$/.test(f));
const unshipped = tracked_docs.filter((f) => !shipped.includes(f));
const missingBuild = MUST_BUILD.filter((f) => !shipped.includes(f));
const leaked = shipped.filter((f) => MUST_NOT.test(f));

if (unshipped.length) {
  console.error(
    `\u2717 ${unshipped.length} page(s) in docs/ that the mirror would not ship:`,
  );
  for (const f of unshipped) console.error(`    ${f}`);
  console.error(
    '  The index links them and the public repository would not have them. ' +
      `Add each to the rsync allowlist in ${WF}.`,
  );
  process.exit(1);
}

if (missingDocs.length || missingBuild.length || leaked.length) {
  if (missingDocs.length) {
    console.error(`\u2717 the mirror would ship no ${missingDocs.length} of the ` +
      `pages a self-hoster needs:`);
    for (const f of missingDocs) console.error(`    ${f}`);
  }
  if (missingBuild.length) {
    console.error(`\u2717 the mirror would not build: ${missingBuild.length} ` +
      `file(s) the image and the stack are made from are missing:`);
    for (const f of missingBuild) console.error(`    ${f}`);
  }
  if (leaked.length) {
    console.error(`\u2717 the mirror would publish ${leaked.length} internal ` +
      `page(s):`);
    for (const f of leaked.slice(0, 8)) console.error(`    ${f}`);
    if (leaked.length > 8) console.error(`    \u2026 and ${leaked.length - 8} more`);
    console.error(`  An rsync allowlist needs a terminating ` +
      `--exclude='/docs/**' after the includes.`);
  }
  process.exit(1);
}

console.log(`\u2713 mirror ships ${shipped.length} files: ${docs.length} docs, ` +
  `the build inputs, nothing internal`);
