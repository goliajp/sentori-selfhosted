// v1.2 W3.a — `sentori-cli upload source-bundle`.
//
// Uploads a pre-built `*.tar.gz` archive of the project's source tree
// for one platform. Server stores it under
// `release_artifacts` with `kind = source_bundle_<platform>`; on
// click-frame, the dashboard's FrameSourceDrawer pulls the matching
// source file out of the archive and shows ±N lines.
//
// We intentionally keep the CLI side small: the operator (or CI)
// already knows how to build a tarball. v1.3 may add an
// auto-bundle-from-directory mode; for now you do:
//
//   tar -czf ios-source.tar.gz Sources/
//   sentori-cli upload source-bundle --project <uuid> \
//     --release myapp@1.0.0 --platform ios --path ios-source.tar.gz
import { spawnSync } from 'node:child_process';
import { mkdtemp, readFile, readdir, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, relative, sep as pathSep } from 'node:path';
const PLATFORMS = new Set(['ios', 'android']);
/** v1.2 W3.d — per-platform file extensions the CLI bundles when
 *  `--from-dir` mode picks files from a project tree. Kept narrow on
 *  purpose: source-bundle is only useful for native source viewing,
 *  and including non-source files would balloon the archive without
 *  helping the lookup path. */
const EXTENSIONS = {
    android: ['.kt', '.java'],
    ios: ['.swift', '.m', '.mm', '.h', '.hpp'],
};
const SKIP_DIRS = new Set([
    '.git',
    'node_modules',
    'Pods',
    'build',
    'DerivedData',
    '.gradle',
    '.build',
    'target',
]);
export async function uploadSourceBundle(opts) {
    if (!PLATFORMS.has(opts.platform)) {
        throw new Error(`--platform must be 'ios' or 'android' (got '${opts.platform}')`);
    }
    if (!opts.release) {
        throw new Error('--release is required for source-bundle uploads');
    }
    // v1.2 W3.d: when `opts.path` is a directory, bundle it on the fly
    // instead of requiring the operator to pre-build the archive.
    let archivePath = opts.path;
    let cleanup;
    try {
        const st = await stat(opts.path);
        if (st.isDirectory()) {
            const built = await buildSourceBundleFromDir(opts.path, opts.platform);
            archivePath = built.path;
            cleanup = built.cleanup;
        }
    }
    catch (e) {
        if (e.code !== 'ENOENT')
            throw e;
        throw new Error(`source path not found: ${opts.path}`);
    }
    try {
        return await uploadPrebuiltTarGz({ ...opts, path: archivePath });
    }
    finally {
        if (cleanup)
            await cleanup();
    }
}
async function uploadPrebuiltTarGz(opts) {
    const body = await readFile(opts.path);
    if (body.length === 0)
        throw new Error(`empty archive: ${opts.path}`);
    // Same gzip magic check the server enforces. Failing early here
    // saves a network round-trip when an operator accidentally points
    // at a raw .tar.
    if (body.length < 2 || body[0] !== 0x1f || body[1] !== 0x8b) {
        throw new Error(`${opts.path} does not look like a gzip stream (expected 1f 8b magic) — ` +
            `did you mean to gzip it first? \`tar -czf <out>.tar.gz <dir>/\``);
    }
    if (!opts.release)
        throw new Error('--release is required for source-bundle uploads');
    const base = opts.apiUrl.replace(/\/+$/, '');
    const q = new URLSearchParams();
    q.set('release', opts.release);
    q.set('platform', opts.platform);
    if (opts.module)
        q.set('module', opts.module);
    const url = `${base}/admin/api/projects/${encodeURIComponent(opts.projectId)}/source-bundles?${q.toString()}`;
    const resp = await fetch(url, {
        body,
        headers: {
            Authorization: `Bearer ${opts.token}`,
            'Content-Type': 'application/gzip',
        },
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
    const parsed = await resp.json();
    if (!parsed ||
        typeof parsed !== 'object' ||
        typeof parsed.contentHash !== 'string') {
        throw new Error(`unexpected response shape: ${JSON.stringify(parsed)}`);
    }
    const r = parsed;
    return { contentHash: r.contentHash, kind: r.kind, sizeBytes: r.sizeBytes };
}
/** Walk `dir`, pick platform-relevant source files, and tar.gz them
 *  into a temp file. Caller is responsible for invoking `cleanup()`.
 *  Uses the system `tar` binary; that's portable enough on macOS +
 *  Linux + WSL, which covers every realistic CI environment Sentori
 *  ships to. Native Windows CI without WSL would need to gzip the
 *  archive themselves (the pre-built-path mode still works). */
export async function buildSourceBundleFromDir(dir, platform) {
    const exts = EXTENSIONS[platform];
    const files = [];
    await walk(dir, dir, exts, files);
    if (files.length === 0) {
        throw new Error(`no ${platform} source files (${exts.join(', ')}) found under ${dir} — ` +
            `pass --platform with the right value or check your source layout`);
    }
    files.sort();
    // Write the file list to a temp file, then have tar consume it via
    // -T —. Avoids the per-arg shell length cap on huge projects.
    const tmp = await mkdtemp(join(tmpdir(), 'sentori-srcbun-'));
    const listPath = join(tmp, 'files.txt');
    const archivePath = join(tmp, `${platform}-source.tar.gz`);
    await writeFile(listPath, files.join('\n'));
    const r = spawnSync('tar', ['-czf', archivePath, '-C', dir, '-T', listPath]);
    if (r.status !== 0) {
        throw new Error(`tar failed (status ${r.status}): ${(r.stderr ?? '').toString().slice(0, 300)}`);
    }
    return {
        cleanup: async () => {
            try {
                await readdir(tmp).then(async (entries) => {
                    for (const e of entries) {
                        await unlinkIfExists(join(tmp, e));
                    }
                });
                await unlinkIfExists(tmp, true);
            }
            catch {
                // best-effort cleanup
            }
        },
        path: archivePath,
    };
}
async function walk(root, dir, exts, out) {
    const entries = await readdir(dir, { withFileTypes: true });
    for (const e of entries) {
        if (e.name.startsWith('.') && e.isDirectory() && e.name !== '.') {
            // Hidden directory: skip (.git, .gradle, .build…). The known
            // ones are listed in SKIP_DIRS for clarity, but any leading-dot
            // dir is also skipped as a safety net.
            continue;
        }
        if (SKIP_DIRS.has(e.name))
            continue;
        const full = join(dir, e.name);
        if (e.isDirectory()) {
            await walk(root, full, exts, out);
        }
        else if (e.isFile()) {
            const lower = e.name.toLowerCase();
            if (exts.some((ext) => lower.endsWith(ext))) {
                out.push(relative(root, full).split(pathSep).join('/'));
            }
        }
    }
}
async function unlinkIfExists(path, isDir = false) {
    const { rm } = await import('node:fs/promises');
    try {
        await rm(path, { force: true, recursive: isDir });
    }
    catch {
        // ignore
    }
}
//# sourceMappingURL=source-bundle.js.map