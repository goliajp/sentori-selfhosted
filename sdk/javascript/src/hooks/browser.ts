import { coerceError } from '@goliapkg/sentori-core'

import { captureError } from '../capture.js'
import { endSession } from '../session-tracker.js'

let installed = false

/**
 * Wire window.onerror + unhandledrejection so uncaught browser errors
 * land as Sentori events automatically. Idempotent — safe to call
 * twice; the second call no-ops.
 */
export function installBrowserHooks(): boolean {
  if (installed) return true
  const w = globalThis as {
    addEventListener?: (
      type: string,
      handler: (e: Event | PromiseRejectionEvent | ErrorEvent) => void
    ) => void
  }
  if (typeof w.addEventListener !== 'function') return false

  // `coerceError` keeps the actual thrown value visible — plain objects
  // come through as JSON instead of `[object Object]`, primitives as
  // their printed value, `{name, message}`-shaped throws preserve both
  // fields. See @goliapkg/sentori-core/coerce-error.
  const onError = (e: Event | ErrorEvent) => {
    const err = (e as ErrorEvent).error
    if (err !== undefined) {
      captureError(coerceError(err))
    } else if (typeof (e as ErrorEvent).message === 'string') {
      captureError(new Error((e as ErrorEvent).message))
    }
  }

  const onRejection = (e: Event | PromiseRejectionEvent) => {
    captureError(coerceError((e as PromiseRejectionEvent).reason))
  }

  w.addEventListener('error', onError)
  w.addEventListener('unhandledrejection', onRejection)
  // Phase 26 sub-B: pagehide is the right unload event in modern
  // browsers (fires on bfcache → background, full unload, and tab
  // close). beforeunload is unreliable on mobile Safari.
  w.addEventListener('pagehide', () => endSession())
  installed = true
  return true
}

/** Test helper — resets the idempotency latch. */
export function _resetBrowserHooksForTesting(): void {
  installed = false
}
