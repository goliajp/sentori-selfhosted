// Sessions — the denominator.
//
// A page view is a session. It ends when the page is hidden, which is
// the only lifecycle moment a browser reliably gives: `beforeunload`
// does not fire on mobile Safari when the tab is swiped away, and
// `unload` breaks the back/forward cache. `visibilitychange` to
// `hidden` fires in every case that matters, so that is where the ping
// goes out.
//
// A crashed session, in a browser, is one that ended with an uncaught
// error and no clean hide. That is weaker evidence than a native crash
// file, so it is recorded as `errored` rather than `crashed` — a
// crash-free rate that counts a caught-and-logged exception as a crash
// disagrees with what the team believes shipped, and then nobody
// trusts the number.

import { SessionTracker, uuidV7 } from '@goliapkg/sentori-core'
import type { SessionContext, SessionPing } from '@goliapkg/sentori-core'

import { currentUserKey } from './scope.js'
import { flush, queueSession } from './transport.js'

let tracker: null | SessionTracker = null
let onHidden: null | (() => void) = null

export const startSession = (ctx: SessionContext): void => {
  tracker ??= new SessionTracker((ping: SessionPing) =>
    // Read at send time: a page learns who the user is after it loads.
    queueSession({ ...ping, userKey: currentUserKey() ?? null })
  )
  tracker.start(ctx)

  if (typeof document === 'undefined' || onHidden) return
  onHidden = (): void => {
    if (document.visibilityState !== 'hidden') return
    tracker?.end('exited')
    // The page may never run our code again, so the ping has to leave
    // now rather than on the next batch timer.
    void flush()
  }
  document.addEventListener('visibilitychange', onHidden)
}

/** An uncaught error happened in this session. */
export const markSessionErrored = (): void => tracker?.markErrored()

export const __resetSessionsForTests = (): void => {
  if (onHidden && typeof document !== 'undefined') {
    document.removeEventListener('visibilitychange', onHidden)
  }
  onHidden = null
  tracker = null
}

export const __peekTrackerForTests = (): null | SessionTracker => tracker
