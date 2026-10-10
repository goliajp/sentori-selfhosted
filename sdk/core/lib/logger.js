/**
 * Sentori SDK internal logger.
 *
 * Design tenet (post-2026-05-23 user feedback):
 *
 *   Sentori is the host's "free bonus" — it MUST NOT pollute the
 *   host's console. Anything the host sees that says `[sentori]`
 *   should mean Sentori has an actual problem, never "Sentori is
 *   doing its job normally."
 *
 * Levels (lowest to highest):
 *
 *   silent → no output at all (use for libraries that test
 *            Sentori inside their own console-capture rigs)
 *   error  → SDK itself broke; report-internal fired; native
 *            module not found; transport sustained failure
 *   warn   → transient operational anomaly; recoverable; one-off
 *   info   → one-line init banner ("sentori: initialized · cold Xms")
 *            and lifecycle moments (flush done on close, etc.)
 *   debug  → everything else: per-tick replay logs, breadcrumb
 *            additions, native method enumeration, transport
 *            retries. Off by default — only host devs debugging
 *            Sentori turn this on.
 *
 * Default level: **`silent`**, including SDK failures. Diagnostics
 * require explicit opt-in and are bounded; they cannot break the host.
 *
 * To opt into more output:
 *
 *     sentori.init({ logLevel: 'debug', ... })
 *
 * To opt into total silence (e.g. CI smoke runs):
 *
 *     sentori.init({ logLevel: 'silent', ... })
 */
const ORDER = {
    silent: 0,
    error: 1,
    warn: 2,
    info: 3,
    debug: 4,
};
const DEFAULT_LEVEL = 'silent';
const MAX_LINES_PER_MINUTE = 20;
const MAX_STRING_LENGTH = 2048;
const MAX_ARGS = 8;
let activeLevel = DEFAULT_LEVEL;
let activeTransport = null;
let transportFailed = false;
let windowStart = 0;
let emitted = 0;
/**
 * Set the active log level. Called once from `init()` after reading
 * the host's config. Idempotent and cheap (one assignment + one
 * read in the level check).
 */
export function setLogLevel(level) {
    activeLevel = typeof level === 'string' && Object.hasOwn(ORDER, level) ? level : DEFAULT_LEVEL;
    windowStart = 0;
    emitted = 0;
}
/** Read-only — surfaced so tests + adapters can inspect. */
export function getLogLevel() {
    return activeLevel;
}
/**
 * Wire a host-supplied logger transport. Pass `null` to restore the
 * default console-based output. See `LogTransport` for the contract.
 */
export function setLogTransport(transport) {
    activeTransport = transport;
    transportFailed = false;
}
function shouldLog(target) {
    return ORDER[target] <= ORDER[activeLevel];
}
/**
 * `[sentori]`-prefixed console call gated by level. Tag is the
 * subsystem (e.g. `'native'`, `'replay'`, `'transport'`) — helps
 * host devs grep but keeps every line consistently formatted.
 */
function log(level, tag, ...args) {
    try {
        if (!shouldLog(level) || transportFailed)
            return;
        const now = Date.now();
        if (now - windowStart >= 60_000 || now < windowStart) {
            windowStart = now;
            emitted = 0;
        }
        if (emitted >= MAX_LINES_PER_MINUTE)
            return;
        emitted += 1;
        // Never enumerate/serialize host objects for diagnostics. Keep work and
        // retained values bounded even with large strings or hostile getters.
        const bounded = args.slice(0, MAX_ARGS).map(value => {
            if (typeof value === 'string')
                return value.slice(0, MAX_STRING_LENGTH);
            if (value === null || ['number', 'boolean', 'undefined'].includes(typeof value))
                return value;
            return '[object omitted]';
        });
        if (activeTransport !== null) {
            const selectedTransport = activeTransport;
            try {
                const result = selectedTransport(level, tag.slice(0, 128), bounded);
                if (result && typeof result.then === 'function') {
                    void Promise.resolve(result).catch(() => {
                        if (activeTransport === selectedTransport)
                            transportFailed = true;
                    });
                }
            }
            catch {
                if (activeTransport === selectedTransport)
                    transportFailed = true;
            }
            return;
        }
        defaultConsoleEmit(level, tag.slice(0, 128), bounded);
    }
    catch {
        // A broken console, clock or host callback must not escape the SDK.
    }
}
function defaultConsoleEmit(level, tag, args) {
    const prefix = `[sentori${tag ? '/' + tag : ''}]`;
    // Choose console method by target level. We deliberately use
    // console.log for debug + info (not console.debug / console.info)
    // because most RN debuggers / browser consoles render those at
    // the same visual weight as `console.log`, while console.warn /
    // console.error get yellow / red highlight that we want reserved
    // for genuine problems.
    switch (level) {
        case 'error':
            // NEVER-rule: Sentori is the host's "free bonus" and must
            // never emit a real `console.error` red-line — even when the
            // SDK itself broke. Host devs see `[sentori]` warn and route
            // it to us; we never want them to mistake an SDK self-report
            // for their own app crashing. Level stays as `error` for any
            // host-supplied transport (which can route it to their
            // aggregator however they like).
            console.warn(prefix, ...args);
            break;
        case 'warn':
            console.warn(prefix, ...args);
            break;
        default:
            console.log(prefix, ...args);
    }
}
export const logger = {
    error: (tag, ...args) => log('error', tag, ...args),
    warn: (tag, ...args) => log('warn', tag, ...args),
    info: (tag, ...args) => log('info', tag, ...args),
    debug: (tag, ...args) => log('debug', tag, ...args),
};
//# sourceMappingURL=logger.js.map