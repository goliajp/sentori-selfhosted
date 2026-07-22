// Phase 35 sub-C follow-up: XMLHttpRequest auto-instrumentation.
//
// The fetch hook (hooks/fetch.ts) only covers callers that use the
// global `fetch`. axios — by far the most common HTTP client on both
// browser and React Native — uses its `xhr` adapter by default, which
// goes through XMLHttpRequest, not fetch. Without this hook, an
// axios-heavy app produces almost no `http.client` spans.
//
// We patch the prototype's `open` / `send` (not subclass the
// constructor) so the same instrumentation works on browsers, RN's
// native XHR polyfill, and any other XHR shim. State is stashed on the
// instance between open() and the terminal `loadend` event.

import { normalizeUrl, startSpan } from '@goliapkg/sentori-core'

import { getConfig } from '../config.js'
import { toTraceparent } from './fetch.js'

function isIngestUrl(url: string): boolean {
  const base = getConfig()?.ingestUrl
  return !!base && url.startsWith(base)
}

type TracedXhr = XMLHttpRequest & {
  __sentoriMethod?: string
  __sentoriSpan?: ReturnType<typeof startSpan>
  __sentoriUrl?: string
}

let _installed = false

export function installXhrInstrumentation(): boolean {
  if (_installed) return true
  const XHR = (globalThis as { XMLHttpRequest?: typeof XMLHttpRequest }).XMLHttpRequest
  if (typeof XHR !== 'function') return false
  const proto = XHR.prototype as XMLHttpRequest & { __sentoriPatched?: boolean }
  if (proto.__sentoriPatched) {
    _installed = true
    return true
  }
  proto.__sentoriPatched = true
  _installed = true

  const originalOpen = proto.open
  const originalSend = proto.send
  const originalSetHeader = proto.setRequestHeader

  proto.open = function (
    this: TracedXhr,
    method: string,
    url: string | URL,
    ...rest: unknown[]
  ): void {
    this.__sentoriMethod = String(method).toUpperCase()
    this.__sentoriUrl = typeof url === 'string' ? url : String(url)
    // @ts-expect-error variadic forwarding to the native open() signature
    return originalOpen.call(this, method, url, ...rest)
  }

  proto.send = function (
    this: TracedXhr,
    body?: Document | XMLHttpRequestBodyInit | null,
  ): void {
    if (isIngestUrl(this.__sentoriUrl ?? '')) return originalSend.call(this, body)
    const method = this.__sentoriMethod ?? 'GET'
    const url = this.__sentoriUrl ?? ''
    const span = startSpan('http.client', {
      name: `${method} ${normalizeUrl(url)}`,
      tags: { 'http.method': method, 'http.url': url },
    })
    this.__sentoriSpan = span

    // setRequestHeader must be called between open() and send() — we
    // are inside send() before the underlying call, so this is legal.
    try {
      originalSetHeader.call(this, 'traceparent', toTraceparent(span.traceId, span.spanId))
    } catch {
      // Strict polyfills may reject post-open header sets; drop the
      // header rather than fail the request.
    }

    this.addEventListener('loadend', () => {
      const s = this.__sentoriSpan
      if (!s) return
      this.__sentoriSpan = undefined
      const status = this.status
      s.setTag('http.status', String(status))
      // status 0 = network error / CORS block / abort. The abort
      // handler below downgrades genuine aborts to "cancelled".
      s.finish({ status: status === 0 || status >= 400 ? 'error' : 'ok' })
    })
    this.addEventListener('abort', () => {
      const s = this.__sentoriSpan
      if (!s) return
      this.__sentoriSpan = undefined
      s.finish({ status: 'cancelled' })
    })

    return originalSend.call(this, body)
  }

  return true
}

/** Test-only: undo the prototype patch. Production never calls this. */
export function uninstallXhrInstrumentation(): void {
  // Prototype patches can't be cleanly reversed without holding the
  // originals at module scope across installs; for tests we just flip
  // the flags so a fresh install no-ops. The patched methods stay,
  // but `__sentoriPatched` being true means a re-install is a no-op,
  // and the patched methods are idempotent w.r.t. span creation
  // anyway (one span per send()). Tests assert on emitted spans, not
  // on globalThis identity, so this is sufficient.
  _installed = false
}
