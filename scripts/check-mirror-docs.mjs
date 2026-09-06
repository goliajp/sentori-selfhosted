// The public mirror must ship the docs a self-hoster needs, and none
// of the ones they must not see.
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
// restatement of them, into a temporary directory.
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, readdirSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
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
];
const MUST_NOT = /^docs\/(archive|design|plans|roadmap|dogfood|performance|perf-baselines|runbook|infrastructure)\//;

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

const out = mkdtempSync(join(tmpdir(), 'sentori-mirror-'));
try {
  // A working tree can hold things rsync cannot copy — a unix socket
  // left by a local dev server makes it exit 23, "partial transfer".
  // CI runs against a clean checkout and will not see them. The docs
  // set is what this checks, so a partial transfer is tolerated and a
  // real failure still is not.
  try {
    execFileSync('rsync', ['-a', '--delete', ...args, './', `${out}/`], {
      cwd: ROOT,
      stdio: 'pipe',
    });
  } catch (e) {
    if (e.status !== 23) throw e;
  }

  const shipped = [];
  const walk = (d) => {
    let entries;
    try { entries = readdirSync(join(out, d)); } catch { return; }
    for (const e of entries) {
      const rel = d ? `${d}/${e}` : e;
      if (statSync(join(out, rel)).isDirectory()) walk(rel);
      else shipped.push(rel);
    }
  };
  walk('docs');

  const missing = MUST_SHIP.filter((f) => !shipped.includes(f));
  const leaked = shipped.filter((f) => MUST_NOT.test(f));

  if (missing.length || leaked.length) {
    if (missing.length) {
      console.error(`✗ the mirror would ship no ${missing.length} of the ` +
        `pages a self-hoster needs:`);
      for (const f of missing) console.error(`    ${f}`);
    }
    if (leaked.length) {
      console.error(`✗ the mirror would publish ${leaked.length} internal ` +
        `page(s):`);
      for (const f of leaked.slice(0, 8)) console.error(`    ${f}`);
      if (leaked.length > 8) console.error(`    … and ${leaked.length - 8} more`);
      console.error(`  An rsync allowlist needs a terminating ` +
        `--exclude='/docs/**' after the includes.`);
    }
    process.exit(1);
  }

  console.log(`✓ mirror ships ${shipped.length} docs, none internal`);
} finally {
  rmSync(out, { recursive: true, force: true });
}
