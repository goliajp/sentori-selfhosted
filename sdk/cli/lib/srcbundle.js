// Source archives are build artifacts. Directory mode needs no repository;
// tag mode reads immutable Git objects without changing the working tree.
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { lstatSync, readFileSync, readdirSync, realpathSync } from 'node:fs';
import { extname, isAbsolute, relative, resolve, sep } from 'node:path';
import { unzipSync, zipSync } from 'fflate';
export const SOURCE_MANIFEST = 'sentori-source-manifest.json';
export const MAX_SOURCE_FILE_BYTES = 512 * 1024;
export const MAX_SOURCE_BYTES = 64 * 1024 * 1024;
const MAX_FILES = 10000;
const SOURCE_EXTS = new Set(['.swift', '.kt', '.kts', '.java', '.m', '.mm', '.h', '.c', '.cc', '.cpp', '.hpp', '.js', '.jsx', '.ts', '.tsx', '.vue', '.svelte', '.rs', '.go', '.py', '.rb', '.php', '.cs', '.dart']);
const SKIP_DIRS = new Set(['node_modules', 'build', 'Pods', 'DerivedData', 'dist', 'target', 'vendor', '__pycache__']);
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');
export function safeSourcePath(path) {
    return path.length > 0 && path.length <= 1024 && !/[\\\x00-\x1f:]/.test(path)
        && !path.startsWith('/') && path.split('/').every(part => part !== '' && part !== '.' && part !== '..');
}
function selected(path, includes, excludes) {
    const under = (parent) => parent === '.' || path === parent || path.startsWith(`${parent}/`);
    return includes.some(under) && !excludes.some(under);
}
function sourcePath(path, includes) {
    return safeSourcePath(path) && SOURCE_EXTS.has(extname(path).toLowerCase())
        && !path.split('/').some((part, index, parts) => part.startsWith('.') || SKIP_DIRS.has(part)
            && !includes.some(include => include === parts.slice(0, index + 1).join('/') || include.startsWith(parts.slice(0, index + 1).join('/') + '/')));
}
function readText(bytes, path) {
    if (bytes.includes(0))
        throw new Error(`source is not UTF-8 text: ${path}`);
    try {
        return new TextDecoder('utf-8', { fatal: true }).decode(bytes);
    }
    catch {
        throw new Error(`source is not UTF-8 text: ${path}`);
    }
}
function selectionPath(root, path) {
    const rel = relative(root, resolve(root, path)).split(sep).join('/') || '.';
    if (rel !== '.' && !safeSourcePath(rel))
        throw new Error(`source path is outside --root: ${path}`);
    return rel;
}
/** Keep every path relative to ONE root, even with several selected directories. */
export function collectSources(paths, options = {}) {
    if (!paths.length)
        throw new Error('choose source directories or files (for example: ios android/app/src shared)');
    const root = realpathSync(resolve(options.root ?? '.'));
    const includes = paths.map(path => selectionPath(root, path));
    const excludes = (options.exclude ?? []).map(path => selectionPath(root, path));
    const bundle = Object.create(null);
    const stats = { files: 0, bytes: 0, skipped: 0 };
    const add = (path, bytes) => {
        if (Object.hasOwn(bundle, path))
            return;
        if (bytes.length > MAX_SOURCE_FILE_BYTES)
            throw new Error(`source exceeds 512 KiB: ${path}; explicitly exclude it if not needed`);
        if (stats.bytes + bytes.length > MAX_SOURCE_BYTES || stats.files >= MAX_FILES)
            throw new Error('source bundle exceeds 64 MiB or 10000 files; narrow the selected directories');
        bundle[path] = readText(bytes, path);
        stats.files++;
        stats.bytes += bytes.length;
    };
    let source = { type: 'directory' };
    if (options.gitTag) {
        const git = (args, limit = MAX_SOURCE_BYTES) => execFileSync('git', ['-C', root, ...args], { maxBuffer: limit, stdio: ['ignore', 'pipe', 'pipe'] });
        const top = realpathSync(git(['rev-parse', '--show-toplevel']).toString().trim());
        if (root !== top)
            throw new Error('--root must be the repository root when using --git-tag');
        const tag = options.gitTag;
        git(['check-ref-format', `refs/tags/${tag}`]);
        const commit = git(['rev-parse', '--verify', `refs/tags/${tag}^{commit}`]).toString().trim();
        if (!/^[0-9a-f]{40,64}$/.test(commit))
            throw new Error('tag did not resolve to a commit');
        source = { type: 'git-tag', tag, commit };
        const records = git(['ls-tree', '-rlz', '--full-tree', commit]).toString().split('\0').filter(Boolean);
        const matched = new Set();
        for (const record of records) {
            const tab = record.indexOf('\t');
            const path = record.slice(tab + 1);
            if (!selected(path, includes, excludes) || !sourcePath(path, includes))
                continue;
            const [mode, type, oid, size] = record.slice(0, tab).trim().split(/\s+/);
            if (type !== 'blob' || !['100644', '100755'].includes(mode)) {
                stats.skipped++;
                continue;
            }
            if (Number(size) > MAX_SOURCE_FILE_BYTES)
                throw new Error(`source exceeds 512 KiB: ${path}`);
            for (const item of includes)
                if (selected(path, [item], []))
                    matched.add(item);
            add(path, git(['cat-file', 'blob', oid], MAX_SOURCE_FILE_BYTES + 1));
        }
        for (const item of includes)
            if (!matched.has(item))
                throw new Error(`no source files at tag ${tag} under ${item}`);
    }
    else {
        const walk = (path) => {
            const full = resolve(root, path);
            const stat = lstatSync(full);
            if (stat.isSymbolicLink()) {
                stats.skipped++;
                return;
            }
            const real = relative(root, realpathSync(full));
            if (real.startsWith(`..${sep}`) || real === '..' || isAbsolute(real))
                throw new Error(`source resolves outside --root: ${path}`);
            if (stat.isDirectory()) {
                for (const entry of readdirSync(full).sort()) {
                    if (entry.startsWith('.') || SKIP_DIRS.has(entry)) {
                        stats.skipped++;
                        continue;
                    }
                    const child = path === '.' ? entry : `${path}/${entry}`;
                    if (excludes.some(x => child === x || child.startsWith(`${x}/`)))
                        continue;
                    walk(child);
                }
            }
            else if (stat.isFile() && sourcePath(path, includes) && selected(path, includes, excludes)) {
                if (stat.size > MAX_SOURCE_FILE_BYTES)
                    throw new Error(`source exceeds 512 KiB: ${path}`);
                add(path, readFileSync(full));
            }
        };
        for (const path of includes)
            walk(path);
    }
    if (!stats.files)
        throw new Error('no supported source files selected; choose the application source directories');
    return { bundle, stats, source };
}
export function packSources(paths, release, options = {}) {
    if (!release.trim())
        throw new Error('release is required and must match the SDK release');
    const { bundle, stats, source } = collectSources(paths, options);
    const entries = Object.create(null);
    const manifest = { version: 1, release, source, files: [] };
    for (const path of Object.keys(bundle).sort()) {
        const bytes = Buffer.from(bundle[path], 'utf8');
        entries[path] = bytes;
        manifest.files.push({ path, bytes: bytes.length, sha256: sha256(bytes) });
    }
    entries[SOURCE_MANIFEST] = Buffer.from(JSON.stringify(manifest, null, 2) + '\n');
    return { bytes: zipSync(entries, { level: 6, mtime: new Date(2000, 0, 1) }), manifest, stats };
}
/** Validate in memory; the server independently repeats validation. */
export function inspectSourceArchive(bytes, release) {
    if (bytes.length > MAX_SOURCE_BYTES + 4 * 1024 * 1024)
        throw new Error('source archive is too large');
    const names = new Set();
    let total = 0;
    const files = unzipSync(bytes, { filter: file => {
            const path = file.name.replace(/\/$/, '');
            if (!safeSourcePath(path) || names.has(path))
                throw new Error(`unsafe or duplicate ZIP path: ${file.name}`);
            names.add(path);
            if (names.size > MAX_FILES + 1)
                throw new Error('ZIP contains too many entries');
            const limit = path === SOURCE_MANIFEST ? 4 * 1024 * 1024 : MAX_SOURCE_FILE_BYTES;
            total += file.originalSize;
            if (file.originalSize > limit || total > MAX_SOURCE_BYTES + 4 * 1024 * 1024)
                throw new Error('ZIP exceeds source size limits');
            return !file.name.endsWith('/');
        } });
    const sources = Object.keys(files).filter(path => path !== SOURCE_MANIFEST);
    if (!sources.length)
        throw new Error('source ZIP is empty');
    for (const path of sources)
        readText(files[path], path);
    let manifest;
    if (files[SOURCE_MANIFEST]) {
        manifest = JSON.parse(readText(files[SOURCE_MANIFEST], SOURCE_MANIFEST));
        if (!manifest || manifest.version !== 1 || typeof manifest.release !== 'string' || !manifest.release.trim() || !Array.isArray(manifest.files)
            || !manifest.source || !['directory', 'git-tag'].includes(manifest.source.type)
            || manifest.files.some(file => !file || typeof file.path !== 'string'))
            throw new Error('invalid source manifest');
        if (release && manifest.release !== release)
            throw new Error(`ZIP release ${manifest.release} does not match ${release}`);
        if (manifest.files.length !== sources.length || new Set(manifest.files.map(file => file.path)).size !== sources.length)
            throw new Error('source manifest file list does not match ZIP');
        for (const file of manifest.files) {
            const body = files[file.path];
            if (!body || body.length !== file.bytes || sha256(body) !== file.sha256)
                throw new Error(`source manifest checksum mismatch: ${file.path}`);
        }
    }
    return { files: sources.sort(), manifest };
}
//# sourceMappingURL=srcbundle.js.map