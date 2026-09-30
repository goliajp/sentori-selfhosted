// Request breadcrumbs, by wrapping `wx.request`.
//
// Every SDK in this ecosystem wraps it, because there is no
// `PerformanceObserver` and no other way to see a request at all. The
// trade the web SDK declines — our code in the path of the host's
// requests — has no alternative here, so it is taken, with the failure
// isolation the React Native network patch uses:
//
//   · the original is called first and its return value is returned
//   · our work happens inside the host's own callbacks, wrapped
//   · anything we throw is swallowed; the host's callback still runs
//
// If our patch breaks, `wx.request` behaves exactly as it did.

import { pushSignal, reportInternal } from '@goliapkg/sentori-core'

import { getWx, type WxRequestOptions } from '../wx.js'

let patched = false

export const installNetworkSignals = (ingestUrl: string): void => {
  const wx = getWx()
  if (patched || !wx) return
  const original = wx.request
  if (typeof original !== 'function') return
  patched = true

  const ownPrefix = ingestUrl

  wx.request = function patchedRequest(options: WxRequestOptions) {
    // Our own ingest calls are not the host's traffic, and recording
    // them would put a breadcrumb of the report before every report.
    if (typeof options?.url === 'string' && ownPrefix && options.url.indexOf(ownPrefix) === 0) {
      return original.call(wx, options)
    }

    const startedAt = Date.now()
    const wrapped: WxRequestOptions = { ...options }

    wrapped.success = (res) => {
      try {
        pushSignal('http', {
          url: stripQuery(options.url),
          method: options.method ?? 'GET',
          // The status is real here, unlike in a browser: `wx.request`
          // hands it back.
          status: res?.statusCode,
          ms: Date.now() - startedAt,
        })
      } catch (e) {
        reportInternal('weapp-request-success', e)
      }
      options.success?.(res)
    }

    wrapped.fail = (err) => {
      try {
        pushSignal('http', {
          url: stripQuery(options.url),
          method: options.method ?? 'GET',
          error: err?.errMsg,
          ms: Date.now() - startedAt,
        })
      } catch (e) {
        reportInternal('weapp-request-fail', e)
      }
      options.fail?.(err)
    }

    return original.call(wx, wrapped)
  }
}

/** A query string carries tokens, ids and search terms; the path is
 *  what identifies the call. */
const stripQuery = (url: string): string => {
  if (typeof url !== 'string') return ''
  const cut = url.indexOf('?')
  return cut === -1 ? url : url.slice(0, cut)
}

export const __resetNetworkForTests = (): void => {
  patched = false
}
