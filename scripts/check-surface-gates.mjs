// Every directory in this repository must be named by the path filter of
// some workflow, or be listed below as deliberately outside them.
//
// `webapp/` reached the cutover with 96 type errors because it was in no
// filter and nothing compiled it. `core/crates/**` was in no native
// filter, so three jobs that build the server never ran for a change to
// the crates it is built from, and sourcemap-e2e missed a `GET releases`
// panic from 2.15.0 to 2.21.1. Both are the same failure: a surface
// arrives, nobody wires it, and every gate stays green by not looking.
//
// Only workflows that run on develop count. Being named by a filter in a
// master-only workflow is what `webapp/**` had, and it meant the first
// build of a dashboard change happened on the branch a green build
// deploys from.
//
// Matching is delegated to git's pathspec rather than a glob matcher
// written here: GitHub's path syntax is the same shape, and a matcher of
// our own would answer "covered" for the cases it got wrong.
import { execFileSync } from 'node:child_process';
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const ROOT = new URL('..', import.meta.url).pathname;
const WF_DIR = join(ROOT, '.github/workflows');

// Surfaces no workflow filter names, each for a reason that has to stay
// true. A new directory is not allowed in here without one.
const OUTSIDE = {
  'brand': 'image assets; nothing to run',
  'ops': 'backup script and a grafana dashboard, neither built here',
  'tools': 'one-off helper, not shipped',
  'docs': 'the unfiltered static checks read every page: links, fences, ' +
    'the API names, the mirror set',
  'deploy': 'the production compose; the unfiltered static checks read ' +
    'its env vars and healthchecks, and deploy.yml installs it',
  '.changeset': 'release metadata consumed by the publish step',
  '.gitignore': 'no job reads it',
  'README.md': 'a document; the mirror check requires it and the link ' +
    'checks read it, both unfiltered',
  'CHANGELOG.md': 'a document, and excluded from the public mirror',
  'NOTICES.md': 'a document',
  'LICENSE-APACHE': 'a licence; the mirror check requires it',
  'LICENSE-MIT': 'a licence; the mirror check requires it',
  'VERSION': 'the unfiltered static checks regenerate the OpenAPI ' +
    'document from it and fail when it drifts',
};

const git = (args) =>
  execFileSync('git', args, { cwd: ROOT, encoding: 'utf8' })
    .split('\n')
    .filter(Boolean);

const tracked = git(['ls-files']);
const surfaceOf = (f) => (f.includes('/') ? f.split('/')[0] : '(root files)');
const surfaces = new Set(tracked.map(surfaceOf));

// Every path a workflow filters on: the `paths:` lists of the triggers
// and the `filters:` blocks of any paths-filter step alike. Only those
// blocks — a quoted item elsewhere (a matrix of package directories, a
// branch pattern) is not a statement that anything gates that path.
const globs = new Set();
for (const f of readdirSync(WF_DIR)) {
  if (!f.endsWith('.yml')) continue;
  const text = readFileSync(join(WF_DIR, f), 'utf8');
  if (!/^\s*branches:.*\bdevelop\b/m.test(text)) continue;
  let depth = null;
  for (const line of text.split('\n')) {
    if (!line.trim() || line.trim().startsWith('#')) continue;
    const indent = line.length - line.trimStart().length;
    if (depth !== null && indent <= depth) depth = null;
    if (/^\s*(paths|filters):\s*(\||>-)?\s*$/.test(line)) { depth = indent; continue; }
    if (depth === null) continue;
    const m = line.match(/^\s*-\s*'?([^'\s][^']*?)'?\s*$/);
    if (m) globs.add(m[1]);
  }
}
if (globs.size < 20) {
  console.error(`✗ found ${globs.size} path globs across the workflows. ` +
    `Broken checker, not a broken repository.`);
  process.exit(1);
}

const covered = new Set();
for (const g of globs) {
  for (const f of git(['ls-files', '--', `:(glob)${g}`])) covered.add(f);
}

const uncovered = [...surfaces]
  .filter((s) => !(s in OUTSIDE))
  .map((s) => [
    s,
    tracked.filter((f) => surfaceOf(f) === s && !covered.has(f) && !(f in OUTSIDE)),
  ])
  .filter(([, files]) => files.length);

// A path filter naming a directory says a job *may* run for it, not
// that any job builds it. `sdk/**` matches every package, while the sdk
// job's matrix names four by hand — so a new package under sdk/ passes
// the check above and is never compiled, tested or size-checked, which
// is the webapp failure one level deeper.
const sdkPackages = tracked
  .filter((f) => /^sdk\/[^/]+\/package\.json$/.test(f))
  .map((f) => f.slice(0, f.lastIndexOf('/')));
const matrix = readFileSync(join(WF_DIR, 'build.yml'), 'utf8');
const unbuilt = sdkPackages.filter((p) => !matrix.includes(`- ${p}\n`));

if (unbuilt.length) {
  console.error(`✗ ${unbuilt.length} package(s) under sdk/ that no job builds:`);
  for (const p of unbuilt) console.error(`    ${p}`);
  console.error(`  The sdk job's matrix in build.yml names its packages one ` +
    `by one. A path filter matching sdk/** is not a job.`);
  process.exit(1);
}

if (uncovered.length) {
  console.error(`✗ ${uncovered.length} surface(s) no workflow path filter ` +
    `names — a change there runs no job that builds it:`);
  for (const [s, files] of uncovered) {
    console.error(`    ${s}/  (${files.length} file(s), e.g. ${files[0]})`);
  }
  console.error(`  Add the path to the workflow that gates it, or say in ` +
    `scripts/check-surface-gates.mjs why it has none.`);
  process.exit(1);
}

const byFilter = [...surfaces].filter((s) =>
  tracked.some((f) => surfaceOf(f) === s && covered.has(f)));
const exemptDirs = [...surfaces].filter((s) => s in OUTSIDE);
const exemptFiles = Object.keys(OUTSIDE).filter((k) => tracked.includes(k));

console.log(`✓ ${surfaces.size} surfaces: ${byFilter.length} named by a ` +
  `workflow filter, ${exemptDirs.length} outside them by declaration, ` +
  `plus ${exemptFiles.length} root files; ${sdkPackages.length} sdk ` +
  `packages all in the build matrix`);
