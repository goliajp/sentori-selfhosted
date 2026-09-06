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
 * Default level: **`warn`**. That means a clean Sentori install
 * shows exactly one line (the init banner) and is otherwise
 * silent. If the host's metro / browser console has `[sentori]`
 * lines beyond that, Sentori is doing something wrong.
 *
 * To opt into more output:
 *
 *     sentori.init({ logLevel: 'debug', ... })
 *
 * To opt into total silence (e.g. CI smoke runs):
 *
 *     sentori.init({ logLevel: 'silent', ... })
 */
export type LogLevel = 'silent' | 'error' | 'warn' | 'info' | 'debug';
/**
 * Host-supplied logger transport. When set via `setLogTransport`,
 * every line ≥ active `logLevel` is dispatched here INSTEAD of to
 * `console.*` — useful for routing Sentori-internal diagnostics
 * into the host's own log aggregator (Datadog, Bugsnag, OpenTelemetry,
 * etc.). Pass `null` to restore console output.
 *
 * The transport itself runs inside SDK code paths — if it throws,
 * it's swallowed and a single one-shot internal warning is logged
 * to console as fallback (NEVER-rule: SDK internals must not
 * propagate to host).
 */
export type LogTransport = (level: LogLevel, tag: string, args: unknown[]) => void;
/**
 * Set the active log level. Called once from `init()` after reading
 * the host's config. Idempotent and cheap (one assignment + one
 * read in the level check).
 */
export declare function setLogLevel(level: LogLevel | undefined): void;
/** Read-only — surfaced so tests + adapters can inspect. */
export declare function getLogLevel(): LogLevel;
/**
 * Wire a host-supplied logger transport. Pass `null` to restore the
 * default console-based output. See `LogTransport` for the contract.
 */
export declare function setLogTransport(transport: LogTransport | null): void;
export declare const logger: {
    error: (tag: string, ...args: unknown[]) => void;
    warn: (tag: string, ...args: unknown[]) => void;
    info: (tag: string, ...args: unknown[]) => void;
    debug: (tag: string, ...args: unknown[]) => void;
};
//# sourceMappingURL=logger.d.ts.map