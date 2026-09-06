// Every admin endpoint scoped to a project must decide whether the
// caller may touch that project.
//
// `admin/releases.rs` shipped three handlers that did not: listing a
// project's releases, listing a release's symbolication artifacts, and
// deleting a release. The first two read across projects; the third
// deleted across them, and `release_artifacts` cascades — so a signed-in
// account assigned to one project could destroy another project's
// ability to symbolicate, permanently. `list_artifacts` even bound the
// path's project as `_project_id`, which is the shape of the mistake:
// the scope was in the URL and nobody read it.
//
// The rule: a handler wired to an /admin/api route whose path or
// signature carries a project or release id must call one of
// ensure_project_access / superadmin_only / is_superadmin.
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';

const ROOT = new URL('../self-hosted/server/src/handlers/', import.meta.url)
  .pathname;
const GUARDS = ['ensure_project_access', 'superadmin_only', 'is_superadmin'];
const MIN_ROUTES = 40;

const mod = readFileSync(join(ROOT, 'mod.rs'), 'utf8');

// route path -> fully-qualified handler paths, admin routes only
const wired = new Map(); // "admin::releases::list" -> Set(paths)
for (const m of mod.matchAll(/\.route\(\s*"(\/admin\/api[^"]*)"\s*,/g)) {
  const path = m[1];
  const rest = mod.slice(m.index + m[0].length);
  const nextRoute = rest.indexOf('.route(');
  const seg = nextRoute === -1 ? rest.slice(0, 400) : rest.slice(0, nextRoute);
  for (const h of seg.matchAll(
    /(?:get|post|put|patch|delete)\(\s*((?:[a-z_]+::)+[a-z_]+)/g,
  )) {
    if (!wired.has(h[1])) wired.set(h[1], new Set());
    wired.get(h[1]).add(path);
  }
}

if (wired.size < MIN_ROUTES) {
  console.error(
    `✗ found only ${wired.size} fully-qualified admin handlers in mod.rs, ` +
      `expected at least ${MIN_ROUTES}. This checker is broken, not the ` +
      `tree — it cannot pass on a list it failed to build.`,
  );
  process.exit(1);
}

// index every `pub async fn` body by module::name
const bodies = new Map();
const walk = (dir) => {
  for (const e of readdirSync(dir)) {
    const p = join(dir, e);
    if (statSync(p).isDirectory()) walk(p);
    else if (e.endsWith('.rs') && e !== 'mod.rs') {
      const src = readFileSync(p, 'utf8');
      const rel = p.slice(ROOT.length).replace(/\.rs$/, '').split('/');
      const mods = rel.join('::');
      const fns = [...src.matchAll(/pub async fn (\w+)\s*\(/g)];
      fns.forEach((f, i) => {
        const end = i + 1 < fns.length ? fns[i + 1].index : src.length;
        bodies.set(`${mods}::${f[1]}`, src.slice(f.index, end));
      });
    }
  }
};
walk(ROOT);

const bad = [];
for (const [handler, paths] of wired) {
  // match on the tail, since mod.rs writes `admin::releases::list`
  const key = [...bodies.keys()].find(
    (k) => k === handler || k.endsWith(handler),
  );
  if (!key) continue; // handler defined elsewhere (e.g. a crate)
  const body = bodies.get(key);
  const sig = body.slice(0, body.indexOf('{'));
  const scoped =
    [...paths].some((p) => /\{(project_id|release_id)\}/.test(p)) ||
    /project_id|release_id/.test(sig);
  if (scoped && !GUARDS.some((g) => body.includes(g))) {
    bad.push({ handler: key, paths: [...paths] });
  }
}

if (bad.length > 0) {
  console.error(
    `✗ ${bad.length} admin handler(s) take a project or release id and ` +
      `never authorise the caller:`,
  );
  for (const b of bad) {
    console.error(`    ${b.handler}`);
    for (const p of b.paths) console.error(`      ${p}`);
  }
  console.error(
    `  Call one of: ${GUARDS.join(', ')}. A route whose path carries a ` +
      `project is not the same as a route that checks it.`,
  );
  process.exit(1);
}

console.log(
  `✓ ${wired.size} admin handlers: every project-scoped one authorises`,
);
