import { afterEach, beforeEach, describe, expect, mock, test } from 'bun:test'

import { addBreadcrumb, clearBreadcrumbs, getBreadcrumbs } from '../src/breadcrumbs.js'
import { captureError, captureMessage, setUser } from '../src/capture.js'
import { setConfig } from '../src/config.js'
import { parseStack } from '../src/stack.js'
import type { Event } from '../src/types.js'
import { uuidV7 } from '../src/uuid.js'

// ── transport mocking ──
let sent: Event[] = []
const fetchMock = mock(async (_url: string | URL | Request, init?: RequestInit) => {
  if (init?.body && typeof init.body === 'string') {
    sent.push(JSON.parse(init.body) as Event)
  }
  return new Response('', { status: 202 })
})
beforeEach(() => {
  sent = []
  fetchMock.mockClear()
  clearBreadcrumbs()
  setUser(null)
  ;(globalThis as { fetch: typeof fetch }).fetch = fetchMock as unknown as typeof fetch
  setConfig({
    enableGlobalHooks: false,
    environment: 'test',
    ingestUrl: 'https://ingest.example.com',
    release: 'myapp@1.2.3+456',
    token: 'st_pk_testtokentoken',
  })
})
afterEach(() => {
  ;(globalThis as { fetch?: typeof fetch }).fetch = undefined as unknown as typeof fetch
})

describe('uuidV7', () => {
  test('produces a v7-shaped id with the version nibble set', () => {
    const id = uuidV7()
    expect(id).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/)
  })

  test('two consecutive ids differ', () => {
    expect(uuidV7()).not.toBe(uuidV7())
  })
})

describe('parseStack', () => {
  test('parses a v8 stack', () => {
    const frames = parseStack(`Error: boom
    at level3 (App.tsx:10:5)
    at level2 (App.tsx:6:10)`)
    expect(frames.length).toBe(2)
    expect(frames[0]?.function).toBe('level3')
    expect(frames[0]?.line).toBe(10)
    expect(frames[0]?.column).toBe(5)
  })

  test('returns empty for missing stack', () => {
    expect(parseStack(undefined)).toEqual([])
  })

  test('strips https:// prefix from file', () => {
    const frames = parseStack(`Error: boom
    at fn (https://example.com/static/App.tsx:1:1)`)
    expect(frames[0]?.file).toBe('static/App.tsx')
  })
})

describe('breadcrumbs', () => {
  test('caps at 100 entries', () => {
    for (let i = 0; i < 110; i++) addBreadcrumb({ data: { i }, type: 'log' })
    const out = getBreadcrumbs()
    expect(out.length).toBe(100)
    // FIFO drop: entry 0..9 should be gone; first should be { i: 10 }
    expect((out[0] as { data: { i: number } }).data.i).toBe(10)
  })
})

describe('captureError', () => {
  test('POSTs an event with parsed stack + user + breadcrumbs', async () => {
    setUser({ anonymous: true, id: 'user-42' })
    addBreadcrumb({ data: { url: '/login' }, type: 'nav' })
    const err = new TypeError('something bad')
    captureError(err, { tags: { plan: 'pro' } })

    // captureError fires-and-forgets; tick the microtask queue.
    await Promise.resolve()
    await Promise.resolve()

    expect(sent.length).toBe(1)
    const ev = sent[0]!
    expect(ev.kind).toBe('error')
    expect(ev.platform).toBe('javascript')
    expect(ev.error.type).toBe('TypeError')
    expect(ev.error.message).toBe('something bad')
    expect(ev.user).toEqual({ anonymous: true, id: 'user-42' })
    expect(ev.tags).toEqual({ plan: 'pro' })
    expect(ev.breadcrumbs.length).toBe(1)
    expect(ev.breadcrumbs[0]?.type).toBe('nav')
    expect(ev.release).toBe('myapp@1.2.3+456')
    expect(ev.environment).toBe('test')
    expect(ev.app.version).toBe('1.2.3')
  })

  test('wraps cause chain', async () => {
    const inner = new Error('root cause')
    const outer = new Error('wrapper')
    ;(outer as { cause?: unknown }).cause = inner
    captureError(outer)
    await Promise.resolve()
    await Promise.resolve()
    expect(sent.length).toBe(1)
    const ev = sent[0]!
    expect(ev.error?.cause?.message).toBe('root cause')
  })

  // Phase 33 sub-D: offline behavior. The JS SDK is fire-and-forget
  // (browser-side; no resident process to retry on). Verify it
  // surfaces the failure to console without crashing the app and
  // does not double-send on a single failure.
  test('does not crash when fetch rejects (offline)', async () => {
    const calls: unknown[] = []
    const warnings: unknown[] = []
    const originalWarn = console.warn
    console.warn = (...args: unknown[]) => warnings.push(args)
    ;(globalThis as { fetch: typeof fetch }).fetch = (async (
      url: string | URL | Request,
    ) => {
      calls.push(String(url))
      throw new TypeError('NetworkError: offline')
    }) as unknown as typeof fetch

    try {
      captureError(new Error('boom while offline'))
      await Promise.resolve()
      await Promise.resolve()
    } finally {
      console.warn = originalWarn
    }

    expect(calls.length).toBe(1)
    expect(warnings.length).toBe(1)
    // v2.3 — log format changed to `[sentori/<subsystem>]` prefix.
    expect(String(warnings[0])).toContain('[sentori/transport]')
  })
})

describe('captureMessage (v2.0)', () => {
  test('POSTs an event with kind=message + level + body', async () => {
    captureMessage('Payment fell back to provider B', { level: 'warning' })
    await Promise.resolve()
    await Promise.resolve()
    expect(sent.length).toBe(1)
    const ev = sent[0]!
    expect(ev.kind).toBe('message')
    expect(ev.level).toBe('warning')
    expect(ev.message).toBe('Payment fell back to provider B')
    expect(ev.error).toBeUndefined()
    expect(ev.release).toBe('myapp@1.2.3+456')
    expect(ev.environment).toBe('test')
  })

  test('defaults level to "info" when omitted', async () => {
    captureMessage('rollout reached threshold')
    await Promise.resolve()
    await Promise.resolve()
    expect(sent.length).toBe(1)
    expect(sent[0]!.level).toBe('info')
  })

  test('merges per-call tags', async () => {
    captureMessage('feature toggled', { level: 'info', tags: { feature: 'dark-mode' } })
    await Promise.resolve()
    await Promise.resolve()
    expect(sent[0]!.tags).toEqual({ feature: 'dark-mode' })
  })

  test('drops empty message strings silently', async () => {
    captureMessage('')
    captureMessage(null as unknown as string)
    captureMessage(undefined as unknown as string)
    await Promise.resolve()
    expect(sent.length).toBe(0)
  })

  // NEVER rule — the safeFn wrapper protects host code from any
  // internal throw. We force one by handing the SDK an unstringify-
  // able payload (circular reference) so the transport's JSON.stringify
  // throws inside the body's send() call chain.
  test('never throws on internal failure (NEVER rule)', async () => {
    const circular: Record<string, unknown> = {}
    circular.self = circular  // boom: JSON.stringify throws TypeError

    expect(() =>
      captureMessage('boom on circular payload', {
        data: circular as Record<string, unknown>,
      }),
    ).not.toThrow()
    // Microtask drain — any swallowed async rejection settles here.
    await Promise.resolve()
    await Promise.resolve()
    // Either nothing landed (sync stringify threw in the wrapped body
    // and safeFn caught it) OR the event landed before the failing
    // serialize step (transport handles its own rejection). Either way
    // the host code didn't throw — that's the NEVER guarantee.
  })
})

describe('beforeSend hook (v2.3)', () => {
  test('mutate path: hook return value lands on the wire', async () => {
    setConfig({
      beforeSend: (ev) => ({ ...ev, tags: { ...ev.tags, scrubbed: '1' } }),
      enableGlobalHooks: false,
      environment: 'test',
      ingestUrl: 'https://ingest.example.com',
      release: 'myapp@1.2.3+456',
      token: 'st_pk_testtokentoken',
    })
    captureMessage('redact me', { tags: { plan: 'pro' } })
    await Promise.resolve()
    await Promise.resolve()
    expect(sent.length).toBe(1)
    expect(sent[0]!.tags?.scrubbed).toBe('1')
    expect(sent[0]!.tags?.plan).toBe('pro')
  })

  test('drop path: returning null suppresses the send', async () => {
    setConfig({
      beforeSend: () => null,
      enableGlobalHooks: false,
      environment: 'test',
      ingestUrl: 'https://ingest.example.com',
      release: 'myapp@1.2.3+456',
      token: 'st_pk_testtokentoken',
    })
    captureMessage('dropped', { tags: { feature: 'x' } })
    captureError(new Error('also dropped'))
    await Promise.resolve()
    await Promise.resolve()
    await Promise.resolve()
    expect(sent.length).toBe(0)
  })

  test('throw path: SDK falls back to unmodified event (NEVER rule)', async () => {
    setConfig({
      beforeSend: () => {
        throw new Error('hook boom')
      },
      enableGlobalHooks: false,
      environment: 'test',
      ingestUrl: 'https://ingest.example.com',
      release: 'myapp@1.2.3+456',
      token: 'st_pk_testtokentoken',
    })
    expect(() => captureMessage('survives a bad hook')).not.toThrow()
    await Promise.resolve()
    await Promise.resolve()
    expect(sent.length).toBe(1)
    expect(sent[0]!.message).toBe('survives a bad hook')
  })

  test('non-event return: SDK falls back to unmodified event', async () => {
    setConfig({
      // @ts-expect-error — host returns garbage; v2.3 contract is "fall back unmodified"
      beforeSend: () => 42,
      enableGlobalHooks: false,
      environment: 'test',
      ingestUrl: 'https://ingest.example.com',
      release: 'myapp@1.2.3+456',
      token: 'st_pk_testtokentoken',
    })
    captureMessage('survives bad return')
    await Promise.resolve()
    await Promise.resolve()
    expect(sent.length).toBe(1)
    expect(sent[0]!.message).toBe('survives bad return')
  })
})
