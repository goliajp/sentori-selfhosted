/**
 * Self-report — observability for SDK-internal failures.
 *
 * When `safeFn` catches an error from inside a public API, it calls
 * `reportInternal(api, err)` so the failure becomes observable in
 * the dashboard without ever surfacing to the host app.
 *
 * Gating: a leaky-bucket circuit breaker caps self-reports at
 * `FAILURE_BUDGET_PER_MIN` per rolling minute. Beyond that the
 * function silently does nothing. This protects the host app from
 * recursion storms: if the transport itself is broken, our
 * self-report attempt could trigger another internal failure, which
 * would trigger another self-report — without the cap we'd spin.
 *
 * The whole body is also wrapped in its own try/catch — recursive
 * failure inside `reportInternal` is silent. The NEVER rule wins
 * over our own observability.
 *
 * `setInternalReporter` is the SDK's hook: the framework SDK (e.g.
 * RN's transport) calls it once during `init()` to wire how the
 * `kind: nearCrash` event actually gets enqueued. Core itself
 * stays transport-agnostic.
 */

import { logger } from './logger.js'

/** Hard cap to prevent recursion storms. Tuned conservatively. */
const FAILURE_BUDGET_PER_MIN = 10

type InternalReporter = (payload: {
  api: string
  message: string
  errorName?: string
  stack?: string
}) => void

let _reporter: InternalReporter | null = null
let _failuresThisMinute = 0
let _windowStart = Date.now()

/** Framework SDK calls this once during `init()` to register the
 *  transport-level enqueue function. Core stays transport-agnostic. */
export function setInternalReporter(reporter: InternalReporter | null): void {
  _reporter = reporter
}

/** Returns true if the circuit is open (we're suppressing reports). */
export function isCircuitOpen(): boolean {
  const now = Date.now()
  if (now - _windowStart > 60_000) {
    _failuresThisMinute = 0
    _windowStart = now
  }
  return _failuresThisMinute >= FAILURE_BUDGET_PER_MIN
}

/** Test hook — reset the circuit breaker so unit tests start fresh. */
export function __resetCircuitForTests(): void {
  _failuresThisMinute = 0
  _windowStart = Date.now()
}

/**
 * Record an internal SDK failure. The host app never sees this
 * call's behaviour — even a recursive throw is swallowed.
 */
export function reportInternal(api: string, err: unknown): void {
  try {
    if (isCircuitOpen()) return
    _failuresThisMinute += 1

    // SDK internal failures route through the v2.3 logger at
    // `error` level. With the default `logLevel: 'warn'` they DO
    // surface (host wants to know if Sentori itself broke). Host
    // can dial up to `silent` to hide everything or down to
    // `debug` to also see successful self-reports.
    logger.error('internal', `failure in ${api}:`, err)

    if (!_reporter) return

    const e = err as Error & { name?: string; message?: string; stack?: string }
    _reporter({
      api,
      message: typeof e?.message === 'string' ? e.message : String(err),
      errorName: typeof e?.name === 'string' ? e.name : undefined,
      stack: typeof e?.stack === 'string' ? e.stack : undefined,
    })
  } catch {
    // Recursive failure during self-report — silent. The contract
    // says NEVER affect host app; even our self-observability
    // doesn't get to recurse.
  }
}
