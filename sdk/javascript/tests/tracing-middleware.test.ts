import { clearSpans, drainSpans } from '@goliapkg/sentori-core'
import { afterEach, beforeEach, describe, expect, test } from 'bun:test'

import {
  expressTracingMiddleware,
  honoTracingMiddleware,
  installFastifyTracing,
  parseTraceparent,
} from '../src/tracing-middleware.js'

beforeEach(() => clearSpans())
afterEach(() => clearSpans())

describe('parseTraceparent', () => {
  test('parses a valid header', () => {
    const r = parseTraceparent(
      '00-0123456789abcdef0123456789abcdef-fedcba9876543210-01',
    )
    expect(r).not.toBeNull()
    expect(r!.traceId).toBe('01234567-89ab-cdef-0123-456789abcdef')
    // 16-hex 'fedcba9876543210' padded with 16 zeros → uuid layout
    expect(r!.spanId).toBe('fedcba98-7654-3210-0000-000000000000')
  })

  test('rejects null / undefined / empty', () => {
    expect(parseTraceparent(null)).toBeNull()
    expect(parseTraceparent(undefined)).toBeNull()
    expect(parseTraceparent('')).toBeNull()
  })

  test('rejects wrong version', () => {
    expect(parseTraceparent('99-' + 'a'.repeat(32) + '-' + 'b'.repeat(16) + '-01')).toBeNull()
  })

  test('rejects wrong field lengths', () => {
    expect(parseTraceparent('00-tooshort-' + 'b'.repeat(16) + '-01')).toBeNull()
    expect(parseTraceparent('00-' + 'a'.repeat(32) + '-tooshort-01')).toBeNull()
  })

  test('rejects non-hex chars', () => {
    expect(parseTraceparent('00-' + 'z'.repeat(32) + '-' + 'b'.repeat(16) + '-01')).toBeNull()
  })

  test('case-insensitive on input, lowercase on output', () => {
    const r = parseTraceparent('00-AABBCCDD' + '00'.repeat(12) + '-CCDDEEFF11223344-01')
    expect(r!.traceId).toMatch(/^[a-f0-9-]+$/)
  })
})

describe('expressTracingMiddleware', () => {
  test('emits http.server span on response finish', () => {
    const mw = expressTracingMiddleware()
    const handlers: Record<string, () => void> = {}
    const req = { headers: {}, method: 'POST', path: '/api/x' }
    const res = {
      on(event: string, fn: () => void) {
        handlers[event] = fn
      },
      statusCode: 0,
    }
    let nextCalled = false
    mw(req, res as never, () => {
      nextCalled = true
    })

    expect(nextCalled).toBe(true)
    expect(drainSpans()).toHaveLength(0) // hasn't fired yet

    // Now simulate response close.
    res.statusCode = 201
    handlers.finish?.()

    const sp = drainSpans()[0]!
    expect(sp.op).toBe('http.server')
    expect(sp.name).toBe('POST /api/x')
    expect(sp.tags).toEqual({
      'http.method': 'POST',
      'http.path': '/api/x',
      'http.status': '201',
    })
    expect(sp.status).toBe('ok')
  })

  test('500 → status=error', () => {
    const mw = expressTracingMiddleware()
    const handlers: Record<string, () => void> = {}
    const res = {
      on(event: string, fn: () => void) {
        handlers[event] = fn
      },
      statusCode: 500,
    }
    mw({ headers: {}, method: 'GET', path: '/' }, res as never, () => {})
    handlers.finish?.()
    expect(drainSpans()[0]?.status).toBe('error')
  })

  test('inherits trace from inbound traceparent header', () => {
    const mw = expressTracingMiddleware()
    const handlers: Record<string, () => void> = {}
    const res = {
      on(event: string, fn: () => void) {
        handlers[event] = fn
      },
      statusCode: 200,
    }
    mw(
      {
        headers: {
          traceparent: '00-0123456789abcdef0123456789abcdef-fedcba9876543210-01',
        },
        method: 'GET',
        path: '/x',
      },
      res as never,
      () => {},
    )
    handlers.finish?.()
    const sp = drainSpans()[0]!
    expect(sp.traceId).toBe('01234567-89ab-cdef-0123-456789abcdef')
    expect(sp.parentSpanId).toBe('fedcba98-7654-3210-0000-000000000000')
  })

  test('double-firing finish + close hooks emits the span once', () => {
    const mw = expressTracingMiddleware()
    const handlers: Record<string, () => void> = {}
    const res = {
      on(event: string, fn: () => void) {
        handlers[event] = fn
      },
      statusCode: 204,
    }
    mw({ headers: {}, method: 'DELETE', path: '/z' }, res as never, () => {})
    handlers.finish?.()
    handlers.close?.()
    expect(drainSpans()).toHaveLength(1)
  })
})

describe('honoTracingMiddleware', () => {
  test('wraps async next() and emits one span', async () => {
    const mw = honoTracingMiddleware()
    const c = {
      req: {
        header: (n: string) => (n === 'traceparent' ? undefined : undefined),
        method: 'GET',
        path: '/health',
      },
      res: { status: 200 },
    }
    let ran = false
    await mw(c, async () => {
      ran = true
    })
    expect(ran).toBe(true)
    const sp = drainSpans()[0]!
    expect(sp.op).toBe('http.server')
    expect(sp.name).toBe('GET /health')
    expect(sp.tags['http.status']).toBe('200')
    expect(sp.status).toBe('ok')
  })

  test('downstream throw → status=error + error.message tag, re-throws', async () => {
    const mw = honoTracingMiddleware()
    const c = {
      req: { header: () => undefined, method: 'GET', path: '/' },
      res: { status: 500 },
    }
    await expect(
      mw(c, async () => {
        throw new TypeError('handler boom')
      }),
    ).rejects.toThrow('handler boom')
    const sp = drainSpans()[0]!
    expect(sp.status).toBe('error')
    expect(sp.tags['error.message']).toContain('handler boom')
  })

  test('parents to inbound traceparent header', async () => {
    const mw = honoTracingMiddleware()
    const c = {
      req: {
        header: (n: string) =>
          n === 'traceparent'
            ? '00-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbb-01'
            : undefined,
        method: 'GET',
        path: '/x',
      },
      res: { status: 200 },
    }
    await mw(c, async () => {})
    const sp = drainSpans()[0]!
    expect(sp.traceId).toBe('aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa')
    expect(sp.parentSpanId).toBe('bbbbbbbb-bbbb-bbbb-0000-000000000000')
  })
})

describe('installFastifyTracing', () => {
  test('registers onRequest + onResponse hooks', () => {
    const registered: Record<string, unknown> = {}
    const fastify = {
      addHook(name: string, h: unknown) {
        registered[name] = h
      },
    }
    installFastifyTracing(fastify as never)
    expect(typeof registered.onRequest).toBe('function')
    expect(typeof registered.onResponse).toBe('function')
  })

  test('span lifecycle across the two hooks', () => {
    const hooks: Record<string, (...a: unknown[]) => void> = {}
    const fastify = {
      addHook(name: string, h: (...a: unknown[]) => void) {
        hooks[name] = h
      },
    }
    installFastifyTracing(fastify as never)

    const req = { headers: {}, method: 'POST', url: '/api/foo' } as Record<
      string,
      unknown
    >
    const reply = { statusCode: 0 } as Record<string, unknown>

    let doneCount = 0
    const done = () => {
      doneCount++
    }

    hooks.onRequest!(req, reply, done)
    // No span yet — fastify emits after onResponse fires.
    expect(drainSpans()).toHaveLength(0)

    reply.statusCode = 503
    hooks.onResponse!(req, reply, done)

    expect(doneCount).toBe(2)
    const sp = drainSpans()[0]!
    expect(sp.op).toBe('http.server')
    expect(sp.name).toBe('POST /api/foo')
    expect(sp.tags['http.status']).toBe('503')
    expect(sp.status).toBe('error')
  })
})
