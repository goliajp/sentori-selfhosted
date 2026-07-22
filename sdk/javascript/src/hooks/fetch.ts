// Phase 35 sub-B: auto-instrument the global `fetch` so every
// outbound HTTP request emits an `http.client` span and propagates a
// W3C `traceparent` header to the receiver.
//
// Idempotent (a second install is a no-op). Uninstall puts the
// original fetch back — only used by tests; production callers
// install once and leave it alone.
//
// XMLHttpRequest is instrumented separately in hooks/xhr.ts — axios
// (default `xhr` adapter) and older XHR-only callers don't go through
// fetch, so the fetch hook alone would miss them.

import { normalizeUrl, startSpan } from '@goliapkg/sentori-core'

import { getConfig } from '../config.js'

// Don't trace requests to our own ingest endpoint — span uploads
// would otherwise spawn http.client spans recursively.
function isIngestUrl(url: string): boolean {
  const base = getConfig()?.ingestUrl
  return !!base && url.startsWith(base)
}

let _originalFetch: typeof fetch | null = null
let _installed = false

export function installFetchInstrumentation(): boolean {
  if (_installed) return true
  if (typeof globalThis.fetch !== 'function') return false
  // Save the raw reference so uninstall can put back the exact same
  // function the host owned. Don't `.bind()` — that would create a
  // new function and break callers who hold the previous reference.
  _originalFetch = globalThis.fetch
  globalThis.fetch = wrappedFetch as typeof fetch
  _installed = true
  return true
}

export function uninstallFetchInstrumentation(): void {
  if (!_installed) return
  if (_originalFetch) {
    globalThis.fetch = _originalFetch
  }
  _installed = false
  _originalFetch = null
}

async function wrappedFetch(
  input: Request | string | URL,
  init?: RequestInit,
): Promise<Response> {
  const original = _originalFetch
  if (!original) {
    // Shouldn't happen — installer holds the ref. Be defensive.
    return globalThis.fetch(input as RequestInfo, init)
  }

  const { method, url } = extractMethodAndUrl(input, init)
  if (isIngestUrl(url)) return original(input as RequestInfo, init)
  const span = startSpan('http.client', {
    name: `${method} ${normalizeUrl(url)}`,
    tags: { 'http.method': method, 'http.url': url },
  })

  // Inject traceparent into outgoing headers. The Headers constructor
  // accepts undefined / Headers / Record / array shapes; normalising
  // here means downstream code sees one shape only.
  const reqInit: RequestInit = { ...(init ?? {}) }
  const headers = mergeHeaders(input, init)
  headers.set('traceparent', toTraceparent(span.traceId, span.spanId))
  reqInit.headers = headers

  try {
    const resp = await original(input as RequestInfo, reqInit)
    span.setTag('http.status', String(resp.status))
    span.finish({
      // 4xx and 5xx both flag the span as error so the trace list's
      // status column reflects the failure. The error/cancelled
      // distinction matches AbortController cancellation in the
      // catch branch below.
      status: resp.status >= 400 ? 'error' : 'ok',
    })
    return resp
  } catch (err) {
    const isAbort = isAbortError(err)
    if (err instanceof Error) {
      span.setTag('error.message', err.message)
    }
    span.finish({ status: isAbort ? 'cancelled' : 'error' })
    throw err
  }
}

function extractMethodAndUrl(
  input: Request | string | URL,
  init?: RequestInit,
): { method: string; url: string } {
  if (typeof input === 'string') {
    return { method: (init?.method ?? 'GET').toUpperCase(), url: input }
  }
  if (input instanceof URL) {
    return { method: (init?.method ?? 'GET').toUpperCase(), url: input.toString() }
  }
  // Request — fetch spec says init.method (if given) overrides
  // request.method.
  return {
    method: (init?.method ?? input.method ?? 'GET').toUpperCase(),
    url: input.url,
  }
}

function mergeHeaders(input: Request | string | URL, init?: RequestInit): Headers {
  // Order: init.headers wins over Request.headers, but we still merge
  // so the wrapper preserves any caller-supplied headers.
  const out = new Headers()
  if (input instanceof Request) {
    input.headers.forEach((v, k) => out.set(k, v))
  }
  if (init?.headers) {
    new Headers(init.headers).forEach((v, k) => out.set(k, v))
  }
  return out
}

function isAbortError(err: unknown): boolean {
  if (typeof err !== 'object' || err === null) return false
  const name = (err as { name?: unknown }).name
  return name === 'AbortError'
}

/**
 * Encode `traceparent` per W3C TraceContext:
 *   00-<32 hex chars: trace-id>-<16 hex chars: parent-id>-01
 *
 * Our internal trace-id is a v7 UUID (32 hex chars total when the
 * dashes are stripped), which fits. Our span-id is also a v7 UUID;
 * the W3C parent-id field is 64 bits / 16 hex, so we truncate to the
 * first 16 hex chars (the high-order bytes — uuid-v7 keeps the
 * timestamp there, which is the most distinguishing prefix).
 *
 * Exported for tests.
 */
export function toTraceparent(traceId: string, spanId: string): string {
  const trace = traceId.replace(/-/g, '').toLowerCase()
  const parent = spanId.replace(/-/g, '').toLowerCase().slice(0, 16)
  return `00-${trace}-${parent}-01`
}
