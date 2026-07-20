import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, rmSync } from 'node:fs';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { uploadSourcemaps } from './upload.js';
/** Resolve `react-native/scripts/compose-source-maps.js` from the
 *  current project's node_modules. Returns null if react-native isn't
 *  installed or the version doesn't ship that script. */
export function resolveComposeScript(fromDir = process.cwd()) {
    const req = createRequire(join(fromDir, 'noop.js'));
    for (const id of [
        'react-native/scripts/compose-source-maps.js',
        '@react-native/community-cli-plugin/dist/utils/composeSourceMaps.js',
    ]) {
        try {
            const p = req.resolve(id);
            if (existsSync(p))
                return p;
        }
        catch {
            // try next
        }
    }
    return null;
}
/** Compose a Metro packager source map + a Hermes source map into a
 *  single map (a temp file the caller is responsible for deleting).
 *  Throws if react-native's compose script can't be found or fails. */
export function composeSourceMaps(metroMap, hermesMap) {
    for (const p of [metroMap, hermesMap]) {
        if (!existsSync(p))
            throw new Error(`no such file: ${p}`);
    }
    const script = resolveComposeScript();
    if (!script) {
        throw new Error("couldn't find react-native's compose-source-maps.js — install react-native, or " +
            'compose the maps yourself and use `sentori-cli upload sourcemap <composed.map>`');
    }
    const out = join(mkdtempSync(join(tmpdir(), 'sentori-rn-')), 'composed.map');
    const r = spawnSync('node', [script, metroMap, hermesMap, '-o', out], { stdio: 'inherit' });
    if (r.status !== 0) {
        throw new Error(`compose-source-maps.js exited with ${r.status ?? 'signal'}`);
    }
    if (!existsSync(out))
        throw new Error('compose-source-maps.js produced no output');
    return out;
}
/** Compose the Metro + Hermes maps, then upload the result (and the
 *  bundle, if given). Cleans up the temp composed map. */
export async function reactNativeUpload(opts) {
    const composed = composeSourceMaps(opts.metroMap, opts.hermesMap);
    try {
        const paths = [composed, ...(opts.bundle ? [opts.bundle] : [])];
        const r = await uploadSourcemaps({
            apiUrl: opts.apiUrl,
            dryRun: opts.dryRun,
            paths,
            release: opts.release,
            token: opts.token,
        });
        return { files: r.files, uploaded: r.uploaded };
    }
    finally {
        try {
            rmSync(composed, { force: true });
            rmSync(join(composed, '..'), { force: true, recursive: true });
        }
        catch {
            // best-effort cleanup
        }
    }
}
//# sourceMappingURL=react-native.js.map