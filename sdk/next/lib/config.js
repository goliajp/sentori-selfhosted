// Phase 21 sub-C: env-driven config resolution.
//
// Next.js convention is `NEXT_PUBLIC_*` for browser-readable values
// and unprefixed for server-only. We honour both — clientInit() reads
// NEXT_PUBLIC_SENTORI_* (the bundler inlines these at build time);
// serverInit() reads SENTORI_* first, falling back to NEXT_PUBLIC_*
// so a single SaaS deploy can share a token between server and client
// when that's desired.
const CLIENT_PREFIX = 'NEXT_PUBLIC_SENTORI_';
const SERVER_PREFIX = 'SENTORI_';
const KEY_MAP = {
    environment: 'ENVIRONMENT',
    ingestUrl: 'INGEST_URL',
    release: 'RELEASE',
    token: 'TOKEN',
};
/**
 * Resolve a complete CommonInitOptions from env + explicit overrides.
 * `side` controls the env prefix; explicit values from `cfg` always
 * win.
 *
 * Throws when a required field is unresolved on either side — the
 * caller can catch + log at boot time and continue without Sentori
 * if the env isn't wired yet.
 */
export function resolveConfig(side, cfg = {}) {
    const env = cfg.envOverride ?? processEnv();
    const out = {};
    for (const k of Object.keys(KEY_MAP)) {
        const explicit = cfg[k];
        if (explicit !== undefined) {
            out[k] = explicit;
            continue;
        }
        const suffix = KEY_MAP[k];
        const browser = env[`${CLIENT_PREFIX}${suffix}`];
        const server = env[`${SERVER_PREFIX}${suffix}`];
        const v = side === 'client' ? browser : (server ?? browser);
        if (v)
            out[k] = v;
    }
    // v2.0 W3 — `capture` is nested, env can't drive it. Carry the
    // explicit value through so callers can still pass
    // `capture: { trackAutoBreadcrumb: true }` to resolveConfig().
    if (cfg.capture !== undefined) {
        out.capture = cfg.capture;
    }
    // Defaults: ingestUrl points at the public SaaS if nothing was set.
    if (!out.ingestUrl)
        out.ingestUrl = 'https://ingest.sentori.golia.jp';
    for (const required of ['environment', 'release', 'token']) {
        if (!out[required]) {
            throw new Error(`[sentori-next] missing config field "${required}" (set ` +
                `${side === 'client' ? CLIENT_PREFIX : SERVER_PREFIX}${KEY_MAP[required]} ` +
                `or pass it explicitly)`);
        }
    }
    return out;
}
function processEnv() {
    // Both Node and browser bundlers expose `process.env` after Next's
    // build pipeline. The browser version only contains NEXT_PUBLIC_*.
    const p = globalThis.process;
    return p?.env ?? {};
}
//# sourceMappingURL=config.js.map