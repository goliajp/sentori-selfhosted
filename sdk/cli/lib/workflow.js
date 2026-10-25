import { createHash } from 'node:crypto';
import { existsSync, lstatSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { basename, dirname, resolve } from 'node:path';
import { gunzipSync } from 'node:zlib';
import { parseArgs } from 'node:util';
import { fetchArtifacts } from './artifacts.js';
import { checkCompatibility } from './compatibility.js';
import { dwarfBinariesIn } from './native-artifacts.js';
import { inspectSourceArchive, packSources } from './srcbundle.js';
import { expandSourcemapPaths, uploadArtifact } from './upload.js';
import { isStrict, lenientFail, stripStrict } from './lenient.js';
const hash = (bytes) => createHash('sha256').update(bytes).digest('hex');
const commonOptions = {
    config: { type: 'string' }, release: { type: 'string' }, 'api-url': { type: 'string' },
    'git-tag': { type: 'string' }, 'dry-run': { type: 'boolean' }, help: { type: 'boolean', short: 'h' },
};
export const WORKFLOW_HELP = `Release uploads:
  sentori init --api-url <url> [--release <r>] [--sourcemap <path>] [--dsym <path>]
               [--mapping <path>] [--sources <dir>] [--git-tag <tag>] [--config <file>]
  sentori config validate [--config sentori.config.json] [--release <r>]
  sentori doctor [--config sentori.config.json] [--api-url <url>]
  sentori upload all [--config sentori.config.json] [--release <r>] [--git-tag <tag>]
                    [--dry-run] [--strict]
  sentori verify [--config sentori.config.json] [--release <r>] [--git-tag <tag>]
  sentori sources pack --release <r> --output sources.zip [--root .] [--git-tag <tag>]
                       [--exclude <path>] <source-dir-or-file...>
  sentori upload srcbundle --release <r> [--root .] [--git-tag <tag>] <sources.zip|source-dir...>

Paths in config are relative to that config. Sources keep paths relative to sources.root.
Set the API-scope token in SENTORI_TOKEN (or the configured tokenEnv); init never stores it.
Release must match the SDK exactly, including its build number. SENTORI_RELEASE is supported.
Git mode reads the specified tag's commit, never the working tree or a guessed latest tag.
Verification checks every planned file's bytes and server validation; it does not claim event matching.
`;
function object(value) { return !!value && typeof value === 'object' && !Array.isArray(value); }
function strings(value, label) {
    if (!Array.isArray(value) || value.some(x => typeof x !== 'string' || !x.trim()))
        throw new Error(`${label} must be an array of paths`);
    return value;
}
export function loadUploadConfig(path) {
    const value = JSON.parse(readFileSync(path, 'utf8'));
    if (!object(value) || value.version !== 1 || typeof value.apiUrl !== 'string' || !object(value.artifacts))
        throw new Error('config requires version: 1, apiUrl, and artifacts; run sentori init');
    if (Object.hasOwn(value, 'token'))
        throw new Error('store the API token in the environment and set tokenEnv, not token in the config');
    const allowed = new Set(['version', 'apiUrl', 'tokenEnv', 'release', 'artifacts', 'sources', 'sourceArchive']);
    for (const key of Object.keys(value))
        if (!allowed.has(key))
            throw new Error(`unknown config field: ${key}`);
    for (const [kind, paths] of Object.entries(value.artifacts)) {
        if (!['sourcemap', 'dsym', 'mapping'].includes(kind))
            throw new Error(`unknown artifact kind ${kind}; use sourcemap, dsym, or mapping`);
        strings(paths, `artifacts.${kind}`);
    }
    if (value.release !== undefined && (typeof value.release !== 'string' || !value.release.trim()))
        throw new Error('release must be a non-empty string');
    if (value.sourceArchive !== undefined && typeof value.sourceArchive !== 'string')
        throw new Error('sourceArchive must be a ZIP path');
    if (value.sources !== undefined) {
        if (!object(value.sources))
            throw new Error('sources must declare include paths');
        for (const key of Object.keys(value.sources))
            if (!['root', 'include', 'exclude', 'gitTag'].includes(key))
                throw new Error(`unknown sources field: ${key}`);
        strings(value.sources.include, 'sources.include');
        if (value.sources.exclude !== undefined)
            strings(value.sources.exclude, 'sources.exclude');
        for (const key of ['root', 'gitTag'])
            if (value.sources[key] !== undefined && typeof value.sources[key] !== 'string')
                throw new Error(`sources.${key} must be a string`);
    }
    if (value.sources && value.sourceArchive)
        throw new Error('choose sources or sourceArchive, not both');
    const tokenEnv = value.tokenEnv ?? 'SENTORI_TOKEN';
    if (typeof tokenEnv !== 'string' || !/^[A-Za-z_][A-Za-z0-9_]*$/.test(tokenEnv))
        throw new Error('tokenEnv must name an environment variable');
    return { ...value, tokenEnv };
}
function apiUrl(raw) {
    const url = new URL(raw);
    if (!['https:', 'http:'].includes(url.protocol) || url.username || url.password || url.search || url.hash)
        throw new Error('apiUrl must be an HTTP(S) instance URL without credentials, query, or fragment');
    return url.toString().replace(/\/+$/, '');
}
function validateArtifact(kind, bytes, name) {
    if (!bytes.length)
        throw new Error(`empty artifact: ${name}`);
    if (kind === 'sourcemap') {
        const map = JSON.parse(bytes.toString('utf8'));
        if (map?.version !== 3 || !(Array.isArray(map.sections) || Array.isArray(map.sources) && typeof map.mappings === 'string'))
            throw new Error(`not a version 3 source map: ${name}`);
    }
    if (kind === 'proguard' && !/^\S.* -> \S+:\s*$/m.test(bytes.toString('utf8')))
        throw new Error(`no ProGuard/R8 class mappings in ${name}`);
    if (kind === 'dsym' && !['feedface', 'cefaedfe', 'feedfacf', 'cffaedfe', 'cafebabe', 'bebafeca', 'cafebabf', 'bfbafeca'].includes(bytes.subarray(0, 4).toString('hex')))
        throw new Error(`not a Mach-O DWARF binary: ${name}`);
}
export function prepareUploadPlan(configPath, overrides = {}) {
    const config = loadUploadConfig(configPath);
    const base = dirname(resolve(configPath));
    const release = overrides.release ?? process.env.SENTORI_RELEASE ?? config.release;
    if (!release?.trim())
        throw new Error('set --release, SENTORI_RELEASE, or config.release to the exact SDK release');
    const plan = { release, apiUrl: apiUrl(overrides.apiUrl ?? process.env.SENTORI_API_URL ?? config.apiUrl), tokenEnv: config.tokenEnv, files: [] };
    const add = (kind, path, name = basename(path)) => {
        let bytes = readFileSync(path);
        if (bytes[0] === 0x1f && bytes[1] === 0x8b) {
            bytes = gunzipSync(bytes, { maxOutputLength: 512 * 1024 * 1024 });
            name = name.replace(/\.gz$/, '');
        }
        validateArtifact(kind, bytes, name);
        const digest = hash(bytes);
        const collision = plan.files.find(file => file.kind === kind && file.name === name);
        if (collision) {
            if (collision.hash !== digest)
                throw new Error(`two different ${kind} files would overwrite ${name}; choose distinct upload paths`);
            return;
        }
        plan.files.push({ kind, path, name, hash: digest, bytes: bytes.length });
    };
    for (const path of config.artifacts.sourcemap ?? []) {
        const maps = expandSourcemapPaths(resolve(base, path));
        if (!maps.length)
            throw new Error(`no source maps under ${path}`);
        for (const file of maps)
            add('sourcemap', file.path, file.name);
    }
    for (const path of config.artifacts.dsym ?? []) {
        const full = resolve(base, path);
        const files = dwarfBinariesIn(full);
        if (!files.length)
            throw new Error(`no dSYM binaries under ${path}`);
        for (const file of files)
            add('dsym', file, lstatSync(full).isDirectory() ? `${basename(full)}-${basename(file)}` : basename(file));
    }
    for (const path of config.artifacts.mapping ?? [])
        add('proguard', resolve(base, path));
    if (config.sources) {
        const packed = packSources(config.sources.include, release, {
            root: resolve(base, config.sources.root ?? '.'), exclude: config.sources.exclude,
            gitTag: overrides.gitTag ?? config.sources.gitTag,
        });
        const dir = resolve(base, '.sentori');
        mkdirSync(dir, { recursive: true });
        const archive = resolve(dir, `sources-${hash(packed.bytes)}.zip`);
        writeFileSync(archive, packed.bytes);
        add('srcbundle', archive, 'sources.zip');
        plan.source = { files: packed.manifest.files.map(file => file.path), tag: packed.manifest.source.tag, commit: packed.manifest.source.commit };
    }
    else if (overrides.gitTag)
        throw new Error('--git-tag requires sources in the config');
    if (config.sourceArchive) {
        const archive = resolve(base, config.sourceArchive);
        const source = inspectSourceArchive(readFileSync(archive), release);
        add('srcbundle', archive);
        plan.source = { files: source.files, tag: source.manifest?.source.tag, commit: source.manifest?.source.commit };
    }
    if (!plan.files.length)
        throw new Error('config selects no files; declare artifacts or sources');
    return plan;
}
export async function verifyUploadPlan(plan, token) {
    const result = await fetchArtifacts({ ...plan, token });
    if (!result.known || result.release !== plan.release)
        return [`release not found: ${plan.release}`];
    const problems = [];
    for (const file of plan.files) {
        const matches = result.artifacts.filter(row => row.kind === file.kind && row.contentHash === file.hash
            && (file.kind === 'dsym' || row.name === file.name));
        if (!matches.length)
            problems.push(`missing exact bytes: ${file.kind} ${file.name}`);
        else if (!matches.some(row => row.usable === true))
            problems.push(`${matches.some(row => row.usable === false) ? 'unreadable' : 'not verified by server'}: ${file.kind} ${file.name}`);
    }
    return problems;
}
export async function runWorkflow(command, argv) {
    const strict = isStrict(argv);
    try {
        const parsed = parseArgs({ args: stripStrict(argv), options: { ...commonOptions,
                ...(command === 'init' ? { sourcemap: { type: 'string', multiple: true }, dsym: { type: 'string', multiple: true }, mapping: { type: 'string', multiple: true }, sources: { type: 'string', multiple: true } } : {}),
            } });
        if (parsed.values.help) {
            console.log(WORKFLOW_HELP);
            return 0;
        }
        if (parsed.values['dry-run'] && command !== 'upload')
            throw new Error('--dry-run is only supported by upload all; doctor and verify must contact the server');
        const configPath = resolve(parsed.values.config ?? 'sentori.config.json');
        if (command === 'init') {
            const v = parsed.values;
            const base = dirname(configPath);
            const config = {
                version: 1, apiUrl: apiUrl(parsed.values['api-url'] ?? process.env.SENTORI_API_URL ?? 'https://sentori.golia.jp'),
                tokenEnv: 'SENTORI_TOKEN', ...(parsed.values.release ? { release: parsed.values.release } : {}),
                artifacts: { sourcemap: v.sourcemap ?? [], dsym: v.dsym ?? [], mapping: v.mapping ?? [] },
            };
            const include = v.sources ?? ['src', 'Sources', 'ios', 'android/app/src'].filter(path => existsSync(resolve(base, path)));
            if (include.length)
                config.sources = { root: '.', include, exclude: [], ...(parsed.values['git-tag'] ? { gitTag: parsed.values['git-tag'] } : {}) };
            else if (parsed.values['git-tag'])
                throw new Error('--git-tag requires --sources paths');
            writeFileSync(configPath, JSON.stringify(config, null, 2) + '\n', { flag: 'wx' });
            console.log(`Created ${configPath}. Review source paths and build artifact paths. Keep .sentori/ out of version control.\nSet SENTORI_TOKEN and the exact SDK release, then run sentori config validate and sentori upload all.`);
            return 0;
        }
        if (command === 'doctor') {
            const config = loadUploadConfig(configPath);
            const token = process.env[config.tokenEnv];
            if (!token)
                throw new Error(`set ${config.tokenEnv} to an API-scope token`);
            const result = await fetchArtifacts({
                apiUrl: apiUrl(parsed.values['api-url'] ?? process.env.SENTORI_API_URL ?? config.apiUrl), token,
                release: parsed.values.release ?? process.env.SENTORI_RELEASE ?? config.release ?? '__sentori_compatibility_check__',
            });
            console.log(checkCompatibility(result, !!(config.sources || config.sourceArchive)));
            console.log(`Node ${process.versions.node}; API token accepted. No files uploaded. Build artifacts and event/build matching were not checked.`);
            return 0;
        }
        const plan = prepareUploadPlan(configPath, { release: parsed.values.release, apiUrl: parsed.values['api-url'], gitTag: parsed.values['git-tag'] });
        console.log(JSON.stringify(plan, null, 2));
        if (command === 'validate' || parsed.values['dry-run']) {
            console.log('Local files checked; no upload or server verification performed.');
            return 0;
        }
        const token = process.env[plan.tokenEnv];
        if (!token)
            throw new Error(`set ${plan.tokenEnv} to an API-scope token`);
        // Check credentials before the first write. An unknown release is allowed.
        console.log(checkCompatibility(await fetchArtifacts({ ...plan, token }), plan.files.some(file => file.kind === 'srcbundle')));
        const failures = [];
        if (command === 'upload') {
            for (const file of plan.files) {
                try {
                    // Prevent edits between planning and upload from invalidating the receipt.
                    let body = readFileSync(file.path);
                    if (body[0] === 0x1f && body[1] === 0x8b)
                        body = gunzipSync(body, { maxOutputLength: 512 * 1024 * 1024 });
                    if (hash(body) !== file.hash)
                        throw new Error('file changed after local validation');
                    const verdict = await uploadArtifact({ ...plan, token, ...file });
                    if (verdict.usable !== true)
                        failures.push(`${file.name}: ${verdict.usable === false ? 'unreadable' : 'not verified by server'}`);
                }
                catch (error) {
                    failures.push(`${file.name}: ${error instanceof Error ? error.message : String(error)}`);
                }
            }
        }
        failures.push(...await verifyUploadPlan(plan, token));
        if (failures.length)
            throw new Error(failures.join('\n'));
        console.log(`Verified ${plan.files.length} exact file(s) for ${plan.release}: stored bytes match and format checks passed. Event/build matching is not asserted.`);
        return 0;
    }
    catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        if (command === 'upload')
            return lenientFail(strict, { failure: message, impact: 'some planned debug files are missing or unverified', retry: 'sentori upload all --config sentori.config.json; sentori verify --config sentori.config.json' });
        console.error(`[sentori] ${message}`);
        return 1;
    }
}
export async function runSourcesPack(argv) {
    try {
        const parsed = parseArgs({ args: argv, allowPositionals: true, options: {
                release: { type: 'string' }, root: { type: 'string' }, output: { type: 'string' },
                'git-tag': { type: 'string' }, exclude: { type: 'string', multiple: true }, help: { type: 'boolean', short: 'h' },
            } });
        if (parsed.values.help) {
            console.log(WORKFLOW_HELP);
            return 0;
        }
        const release = parsed.values.release ?? process.env.SENTORI_RELEASE ?? '';
        if (!parsed.values.output)
            throw new Error('--output <sources.zip> is required');
        const options = { root: parsed.values.root, gitTag: parsed.values['git-tag'], exclude: parsed.values.exclude };
        const packed = packSources(parsed.positionals, release, options);
        writeFileSync(parsed.values.output, packed.bytes, { flag: 'wx' });
        console.log(JSON.stringify({ output: parsed.values.output, ...packed.manifest, stats: packed.stats }, null, 2));
        return 0;
    }
    catch (error) {
        console.error(`[sentori] ${error instanceof Error ? error.message : String(error)}`);
        return 1;
    }
}
//# sourceMappingURL=workflow.js.map