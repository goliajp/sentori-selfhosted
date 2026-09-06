// `sentori-cli probes sync` — the tripwire registry scan
// (design.md §2). Statically scans source for `sentori.probe('REF')`
// / `probe("REF")` call sites and registers the refs against a
// release, so the server can tell a silent probe (fix holding) from
// deleted code.
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
const PROBE_RE = /\bprobe\(\s*['"`]([^'"`]{1,200})['"`]/g;
const SCAN_EXTS = new Set(['.ts', '.tsx', '.js', '.jsx', '.mjs', '.cjs']);
const SKIP_DIRS = new Set(['node_modules', '.git', 'lib', 'dist', 'build', '.expo']);
const MAX_FILES = 20_000;
export function scanProbes(root) {
    const refs = new Set();
    let seen = 0;
    const walk = (dir) => {
        if (seen > MAX_FILES)
            return;
        let entries;
        try {
            entries = readdirSync(dir);
        }
        catch {
            return;
        }
        for (const name of entries) {
            if (SKIP_DIRS.has(name))
                continue;
            const p = join(dir, name);
            let st;
            try {
                st = statSync(p);
            }
            catch {
                continue;
            }
            if (st.isDirectory()) {
                walk(p);
            }
            else if (st.isFile()) {
                const dot = name.lastIndexOf('.');
                if (dot === -1 || !SCAN_EXTS.has(name.slice(dot)))
                    continue;
                seen += 1;
                try {
                    const text = readFileSync(p, 'utf8');
                    for (const m of text.matchAll(PROBE_RE)) {
                        const ref = m[1];
                        if (ref)
                            refs.add(ref);
                    }
                }
                catch {
                    // unreadable file — skip
                }
            }
        }
    };
    walk(root);
    return [...refs].sort();
}
export async function syncProbes(opts) {
    const resp = await fetch(`${opts.apiUrl.replace(/\/+$/, '')}/api/probes:sync`, {
        method: 'POST',
        headers: {
            Authorization: `Bearer ${opts.token}`,
            'Content-Type': 'application/json',
        },
        body: JSON.stringify({ release: opts.release, refs: opts.refs }),
    });
    if (!resp.ok) {
        throw new Error(`probes:sync ${resp.status} ${await resp.text().catch(() => '')}`);
    }
    return (await resp.json());
}
//# sourceMappingURL=probes.js.map