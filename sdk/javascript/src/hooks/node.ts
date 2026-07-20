import { coerceError } from '@goliapkg/sentori-core'

import { captureError } from '../capture.js'
import { endSession } from '../session-tracker.js'

let installed = false

/**
 * Wire process.on('uncaughtException') + 'unhandledRejection'.
 * Idempotent. Returns false if not running on Node (no `process.on`).
 *
 * Node policy notes:
 *   - We do NOT call process.exit on uncaughtException; Sentori doesn't
 *     own the host's crash strategy. The host's existing handler
 *     (default: log + exit 1) runs after ours.
 *   - Bun + Deno expose process.on for compatibility; the same code
 *     path covers them.
 */
export function installNodeHooks(): boolean {
  if (installed) return true
  const p = (globalThis as { process?: NodeJS.Process }).process
  if (!p || typeof p.on !== 'function') return false

  p.on('uncaughtException', (err: unknown) => {
    captureError(coerceError(err))
  })
  p.on('unhandledRejection', (reason: unknown) => {
    captureError(coerceError(reason))
  })
  // Phase 26 sub-B: ship a session ping on graceful exit.
  // beforeExit fires when the loop is about to drain — our last
  // chance to send while fetch is still functional.
  p.on('beforeExit', () => endSession('exited'))
  installed = true
  return true
}

export function _resetNodeHooksForTesting(): void {
  installed = false
}
