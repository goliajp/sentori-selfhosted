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

export type LogLevel = 'silent' | 'error' | 'warn' | 'info' | 'debug'

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
export type LogTransport = (level: LogLevel, tag: string, args: unknown[]) => void

const ORDER: Record<LogLevel, number> = {
  silent: 0,
  error: 1,
  warn: 2,
  info: 3,
  debug: 4,
}

const DEFAULT_LEVEL: LogLevel = 'warn'

let activeLevel: LogLevel = DEFAULT_LEVEL
let activeTransport: LogTransport | null = null
let transportThrewWarned = false

/**
 * Set the active log level. Called once from `init()` after reading
 * the host's config. Idempotent and cheap (one assignment + one
 * read in the level check).
 */
export function setLogLevel(level: LogLevel | undefined): void {
  activeLevel = level ?? DEFAULT_LEVEL
}

/** Read-only — surfaced so tests + adapters can inspect. */
export function getLogLevel(): LogLevel {
  return activeLevel
}

/**
 * Wire a host-supplied logger transport. Pass `null` to restore the
 * default console-based output. See `LogTransport` for the contract.
 */
export function setLogTransport(transport: LogTransport | null): void {
  activeTransport = transport
  transportThrewWarned = false
}

function shouldLog(target: LogLevel): boolean {
  return ORDER[target] <= ORDER[activeLevel]
}

/**
 * `[sentori]`-prefixed console call gated by level. Tag is the
 * subsystem (e.g. `'native'`, `'replay'`, `'transport'`) — helps
 * host devs grep but keeps every line consistently formatted.
 */
function log(level: LogLevel, tag: string, ...args: unknown[]): void {
  if (!shouldLog(level)) return

  if (activeTransport !== null) {
    try {
      activeTransport(level, tag, args)
    } catch (e) {
      // NEVER-rule: host transport throwing must not propagate.
      // One-shot console fallback so host devs notice the bug
      // without each subsequent log re-warning.
      if (!transportThrewWarned) {
        transportThrewWarned = true
        try {
          console.warn(
            '[sentori/logger] setLogTransport callback threw; falling back to console for this line and onward',
            e,
          )
        } catch {
          // console itself unavailable — give up silently.
        }
      }
      // Fall through to console so this line isn't lost.
      defaultConsoleEmit(level, tag, args)
    }
    return
  }

  defaultConsoleEmit(level, tag, args)
}

function defaultConsoleEmit(level: LogLevel, tag: string, args: unknown[]): void {
  const prefix = `[sentori${tag ? '/' + tag : ''}]`
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
      console.warn(prefix, ...args)
      break
    case 'warn':
      console.warn(prefix, ...args)
      break
    default:
      console.log(prefix, ...args)
  }
}

export const logger = {
  error: (tag: string, ...args: unknown[]) => log('error', tag, ...args),
  warn: (tag: string, ...args: unknown[]) => log('warn', tag, ...args),
  info: (tag: string, ...args: unknown[]) => log('info', tag, ...args),
  debug: (tag: string, ...args: unknown[]) => log('debug', tag, ...args),
}
