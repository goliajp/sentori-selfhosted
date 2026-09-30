// Requests, as breadcrumbs.
//
// `PerformanceObserver` on `resource` entries, not a patch of `fetch`
// and `XMLHttpRequest`. The browser is already recording these, so
// reading them costs nothing the page was not already paying, and a
// bug in our code cannot break the host's requests — which is the
// failure mode that matters, and the one a patch always risks.
//
// **What this cannot see: the status code.** A resource timing entry
// has timings, a transfer size and a URL, and no HTTP status. A 500
// and a 200 look identical here. The React Native SDK patches `fetch`
// and does have the status; the web SDK does not, and the docs say so
// rather than leaving a reader to assume the field is just missing
// from this particular entry.
//
// `responseStatus` exists in Chromium and is on no other engine, so it
// is read when present and never relied on.

import { pushSignal, reportInternal } from '@goliapkg/sentori-core'

let observer: null | PerformanceObserver = null

/** Our own ingest calls are not the host's traffic, and reporting them
 *  would make every error carry a breadcrumb of the report before it. */
let ownOrigin = ''

export const installNetworkSignals = (ingestUrl: string): void => {
  if (observer || typeof PerformanceObserver === 'undefined') return
  try {
    ownOrigin = new URL(ingestUrl).origin
  } catch {
    ownOrigin = ''
  }
  try {
    observer = new PerformanceObserver((list) => {
      for (const raw of list.getEntries()) {
        const e = raw as PerformanceResourceTiming & { responseStatus?: number }
        if (e.initiatorType !== 'fetch' && e.initiatorType !== 'xmlhttprequest') continue
        if (ownOrigin && e.name.startsWith(ownOrigin)) continue
        pushSignal('http', {
          url: stripQuery(e.name),
          ms: Math.round(e.duration),
          // Chromium only. Absent elsewhere, and absent is honest —
          // a zero here would read as a failed request.
          ...(typeof e.responseStatus === 'number' ? { status: e.responseStatus } : {}),
          bytes: e.transferSize || undefined,
        })
      }
    })
    // `buffered` picks up the requests the page made before `init`
    // ran, which is most of them on a cold load.
    observer.observe({ type: 'resource', buffered: true })
  } catch (err) {
    reportInternal('network-observer', err)
    observer = null
  }
}

/** A query string carries session tokens, search terms and ids. The
 *  path is what identifies the call. */
const stripQuery = (url: string): string => {
  const cut = url.indexOf('?')
  return cut === -1 ? url : url.slice(0, cut)
}

export const uninstallNetworkSignals = (): void => {
  try {
    observer?.disconnect()
  } catch {
    // Disconnecting a dead observer is not worth a word.
  }
  observer = null
}
