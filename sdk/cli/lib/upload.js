import { readFile, readdir, stat } from 'node:fs/promises';
import { basename, join } from 'node:path';
// Files worth uploading: source maps, and the bundle they map (so the
// dashboard can show the minified line too if a frame falls outside
// the map). `.jsbundle` (iOS) and `.bundle` (Android) are RN's bundle
// names; `.hbc` is the Hermes bytecode bundle.
const UPLOADABLE = /\.(map|js|jsbundle|bundle|hbc)$/i;
/** Resolve `paths` (files or dirs) to a deduped list of files to upload.
 *  A directory contributes its top-level `.map` / `.js` / `.bundle` /
 *  `.hbc` files; a file given explicitly is taken as-is (even if its
 *  extension isn't in the list — the caller asked for it). */
export async function collectFiles(paths) {
    const out = [];
    for (const p of paths) {
        const s = await stat(p).catch(() => null);
        if (!s)
            throw new Error(`no such file or directory: ${p}`);
        if (s.isDirectory()) {
            for (const entry of await readdir(p)) {
                const full = join(p, entry);
                const es = await stat(full).catch(() => null);
                if (es?.isFile() && UPLOADABLE.test(entry))
                    out.push(full);
            }
        }
        else {
            out.push(p);
        }
    }
    const deduped = [...new Set(out)];
    if (deduped.length === 0) {
        throw new Error('no .map / .js / .bundle files found in the given path(s)');
    }
    return deduped;
}
export async function uploadSourcemaps(opts) {
    const files = await collectFiles(opts.paths);
    if (opts.dryRun)
        return { files };
    const form = new FormData();
    for (const f of files) {
        const buf = await readFile(f);
        form.append('file', new Blob([buf]), basename(f));
    }
    const base = opts.apiUrl.replace(/\/+$/, '');
    const url = `${base}/admin/api/releases/${encodeURIComponent(opts.release)}/sourcemaps`;
    const resp = await fetch(url, {
        body: form,
        headers: { Authorization: `Bearer ${opts.token}` },
        method: 'POST',
    });
    if (!resp.ok) {
        let detail = '';
        try {
            detail = await resp.text();
        }
        catch {
            // ignore
        }
        throw new Error(`${resp.status} ${resp.statusText}${detail ? ` — ${detail.slice(0, 300)}` : ''}`);
    }
    const body = (await resp.json().catch(() => ({})));
    return { artifacts: body.artifacts, files, uploaded: body.uploaded ?? files.length };
}
//# sourceMappingURL=upload.js.map