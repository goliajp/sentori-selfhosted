import { clearSpans, drainSpans } from '@goliapkg/sentori-core'
import { afterEach, beforeEach, describe, expect, test } from 'bun:test'

import { installXhrInstrumentation, uninstallXhrInstrumentation } from '../src/hooks/xhr.js'

// Minimal fake XMLHttpRequest. The hook patches XHR.prototype.open /
// .send, so as long as our fake's prototype carries those methods +
// addEventListener + setRequestHeader + a `status` field, the
// instrumentation works the same as against the browser/RN native XHR.

type Listener = () => void

class FakeXHR {
  status = 0
  private listeners: Record<string, Listener[]> = {}
  private requestHeaders: Record<string, string> = {}
  private opened = false

  open(_method: string, _url: string | URL): void {
    this.opened = true
  }

  setRequestHeader(name: string, value: string): void {
    if (!this.opened) throw new Error('setRequestHeader before open')
    this.requestHeaders[name.toLowerCase()] = value
  }

  send(_body?: unknown): void {
    // no-op; tests drive the lifecycle manually via fire()
  }

  addEventListener(event: string, fn: Listener): void {
    ;(this.listeners[event] ??= []).push(fn)
  }

  // ── test helpers ──
  getHeader(name: string): string | undefined {
    return this.requestHeaders[name.toLowerCase()]
  }

  fire(event: string): void {
    for (const fn of this.listeners[event] ?? []) fn()
  }
}

let originalXHR: unknown
beforeEach(() => {
  clearSpans()
  originalXHR = (globalThis as { XMLHttpRequest?: unknown }).XMLHttpRequest
  ;(globalThis as { XMLHttpRequest: unknown }).XMLHttpRequest = FakeXHR as unknown
})
afterEach(() => {
  uninstallXhrInstrumentation()
  ;(globalThis as { XMLHttpRequest?: unknown }).XMLHttpRequest = originalXHR
  clearSpans()
  // Reset the prototype-patched flag so the next test's install runs.
  delete (FakeXHR.prototype as { __sentoriPatched?: boolean }).__sentoriPatched
})

describe('installXhrInstrumentation', () => {
  test('returns false when XMLHttpRequest is unavailable', () => {
    ;(globalThis as { XMLHttpRequest?: unknown }).XMLHttpRequest = undefined
    expect(installXhrInstrumentation()).toBe(false)
  })

  test('idempotent — second install is a no-op', () => {
    expect(installXhrInstrumentation()).toBe(true)
    expect(installXhrInstrumentation()).toBe(true)
    // One open/send pair → exactly one span, not two.
    const x = new FakeXHR()
    x.open('GET', 'https://api.example.com/x')
    x.send()
    x.status = 200
    x.fire('loadend')
    expect(drainSpans()).toHaveLength(1)
  })
})

describe('patched XHR lifecycle', () => {
  test('emits http.client span on loadend', () => {
    installXhrInstrumentation()
    const x = new FakeXHR()
    x.open('POST', 'https://api.example.com/v1/orders')
    x.send('{}')
    x.status = 201
    x.fire('loadend')

    const sp = drainSpans()[0]!
    expect(sp.op).toBe('http.client')
    expect(sp.name).toBe('POST https://api.example.com/v1/orders')
    expect(sp.tags).toMatchObject({
      'http.method': 'POST',
      'http.status': '201',
      'http.url': 'https://api.example.com/v1/orders',
    })
    expect(sp.status).toBe('ok')
  })

  test('injects W3C traceparent request header', () => {
    installXhrInstrumentation()
    const x = new FakeXHR()
    x.open('GET', 'https://api.example.com/x')
    x.send()
    const tp = x.getHeader('traceparent')
    expect(tp).toBeDefined()
    expect(tp).toMatch(/^00-[0-9a-f]{32}-[0-9a-f]{16}-01$/)
    x.status = 200
    x.fire('loadend')
  })

  test('5xx → span.status = "error"', () => {
    installXhrInstrumentation()
    const x = new FakeXHR()
    x.open('GET', 'https://api.example.com/x')
    x.send()
    x.status = 503
    x.fire('loadend')
    expect(drainSpans()[0]?.status).toBe('error')
  })

  test('status 0 (network error / CORS) → span.status = "error"', () => {
    installXhrInstrumentation()
    const x = new FakeXHR()
    x.open('GET', 'https://api.example.com/x')
    x.send()
    x.status = 0
    x.fire('loadend')
    expect(drainSpans()[0]?.status).toBe('error')
  })

  test('abort event → span.status = "cancelled"', () => {
    installXhrInstrumentation()
    const x = new FakeXHR()
    x.open('GET', 'https://api.example.com/x')
    x.send()
    x.fire('abort')
    expect(drainSpans()[0]?.status).toBe('cancelled')
  })

  test('open() with URL instance', () => {
    installXhrInstrumentation()
    const x = new FakeXHR()
    x.open('PATCH', new URL('https://api.example.com/u'))
    x.send()
    x.status = 200
    x.fire('loadend')
    expect(drainSpans()[0]?.name).toBe('PATCH https://api.example.com/u')
  })

  test('two concurrent requests → two independent spans', () => {
    installXhrInstrumentation()
    const a = new FakeXHR()
    const b = new FakeXHR()
    a.open('GET', 'https://api.example.com/a')
    b.open('GET', 'https://api.example.com/b')
    a.send()
    b.send()
    a.status = 200
    b.status = 404
    a.fire('loadend')
    b.fire('loadend')
    const spans = drainSpans()
    expect(spans).toHaveLength(2)
    expect(spans.find((s) => s.name.endsWith('/a'))?.status).toBe('ok')
    expect(spans.find((s) => s.name.endsWith('/b'))?.status).toBe('error')
  })
})
