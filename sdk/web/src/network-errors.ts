import { safeFn } from '@goliapkg/sentori-core'
import { normalizeRoute } from './performance-model.js'

/** Optional observations: 5xx and connectivity failures. Aborts, opaque responses,
 * 4xx and request/response bodies are deliberately excluded. */
export function installNetworkErrors(endpoint: string, report: (kind: string, route: string, status?: number) => void): () => void {
  let active = true, count = 0
  const original = window.fetch
  const notify = safeFn('web.networkError', (kind: string, raw: string, status?: number) => {
    if (!active || count >= 10) return
    const url = new URL(raw, location.href)
    if (!['http:', 'https:'].includes(url.protocol) || url.href.startsWith(endpoint)) return
    count++; report(kind, normalizeRoute(url.href), status)
  })
  const wrapped = function (this: unknown, input: RequestInfo | URL, init?: RequestInit) {
    const promise = original.call(this, input, init)
    try {
      const url = typeof input === 'string' ? input : input instanceof URL ? input.href : input.url
      return promise.then(response => {
        try { if (response.status >= 500 && response.status <= 599) notify('http',url,response.status) } catch { /* Preserve the response. */ }
        return response
      }, reason => {
        try { if (reason?.name !== 'AbortError' && !init?.signal?.aborted && !(input instanceof Request && input.signal.aborted)) notify('connectivity',url) } catch { /* Preserve the rejection. */ }
        // Keep an ignored rejection observable as unhandledrejection on the promise returned to the host.
        throw reason
      })
    } catch { /* Instrumentation failure leaves the original promise untouched. */ }
    return promise
  }
  Object.setPrototypeOf(wrapped, original)
  if (typeof XMLHttpRequest === 'undefined') { window.fetch = wrapped as typeof fetch; return () => { active = false; if (window.fetch === wrapped) window.fetch = original } }
  const proto = XMLHttpRequest.prototype, open = proto.open, send = proto.send
  const urls = new WeakMap<XMLHttpRequest,string>(), pending = new Map<XMLHttpRequest,()=>void>()
  const wrappedOpen = function (this: XMLHttpRequest, ...args: [string, string | URL, boolean?, (string | null)?, (string | null)?]) {
    const result = Reflect.apply(open,this,args)
    try { if (typeof args[1] === 'string' || args[1] instanceof URL) urls.set(this,String(args[1])) } catch { /* No host impact. */ }
    return result
  }
  const wrappedSend: typeof send = function (this: XMLHttpRequest, ...args) {
    if (active && pending.size < 50) {
      try {
        let aborted = false, failed = false
        const abort = () => { aborted = true }, failure = () => { failed = true }
        const clean = () => { this.removeEventListener('abort',abort);this.removeEventListener('error',failure);this.removeEventListener('loadend',end);pending.delete(this) }
        const end = safeFn('web.xhrError', () => {
          clean(); if (aborted) return
          const url = urls.get(this) ?? ''
          if (failed) notify('connectivity',url)
          else if (this.status >= 500 && this.status <= 599) notify('http',url,this.status)
        })
        pending.get(this)?.()
        pending.set(this,clean)
        this.addEventListener('abort',abort);this.addEventListener('error',failure);this.addEventListener('loadend',end)
      } catch { try { pending.get(this)?.() } catch { /* Optional cleanup. */ } }
    }
    try { return send.apply(this,args) }
    catch (error) { try { pending.get(this)?.() } catch { /* Preserve the original exception. */ } throw error }
  }
  const cleanup = () => {
    active = false
    try { if (window.fetch === wrapped) window.fetch = original } catch { /* Another owner may have frozen it. */ }
    try { if (proto.open === wrappedOpen) proto.open = open } catch { /* Another owner may have frozen it. */ }
    try { if (proto.send === wrappedSend) proto.send = send } catch { /* Another owner may have frozen it. */ }
    for (const clean of pending.values()) { try { clean() } catch { /* Optional cleanup. */ } }
    pending.clear()
  }
  try { window.fetch = wrapped as typeof fetch; proto.open = wrappedOpen; proto.send = wrappedSend } catch { cleanup() }
  return cleanup
}
