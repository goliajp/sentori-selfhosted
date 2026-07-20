/**
 * Phase 26 sub-B: SDK-side session tracking glue.
 *
 * Holds a singleton `SessionTracker` keyed off the package's lifetime.
 * `initSentori` calls `start()`; capture promotes status; pagehide /
 * graceful Node exit calls `end()`.
 */

import { SessionTracker } from '@goliapkg/sentori-core'

import { getUser } from './capture.js'
import { getConfig } from './config.js'
import { sendSession } from './transport.js'

let _tracker: null | SessionTracker = null

function tracker(): SessionTracker {
  if (_tracker) return _tracker
  _tracker = new SessionTracker((ping) => {
    const cfg = getConfig()
    if (!cfg) return
    void sendSession({ ingestUrl: cfg.ingestUrl, token: cfg.token }, ping)
  })
  return _tracker
}

export function startSession(): void {
  const cfg = getConfig()
  if (!cfg) return
  const user = getUser()
  tracker().start({
    environment: cfg.environment,
    release: cfg.release,
    userId: user?.id ?? null,
  })
}

export function endSession(status?: 'exited'): void {
  if (!_tracker) return
  _tracker.end(status)
}

export function markSessionErrored(): void {
  _tracker?.markErrored()
}

export function markSessionCrashed(): void {
  _tracker?.markCrashed()
}

/** Test helper — drops the singleton so each test starts clean. */
export function _resetSessionTrackerForTesting(): void {
  _tracker = null
}
