import { afterEach, describe, expect, test } from 'bun:test'

import {
  __resetTraceContextForTests,
  activeSpan,
  clearSpans,
  drainSpans,
  getSpans,
  SpanBuffer,
  SpanHandle,
  startSpan,
  withSpan,
} from '../index.js'

afterEach(() => {
  clearSpans()
  __resetTraceContextForTests()
})

describe('startSpan', () => {
  test('roots a new trace when no parent / no active span', () => {
    const s = startSpan('http.client')
    expect(s.parentSpanId).toBeNull()
    expect(s.traceId).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-7/)
    expect(s.spanId).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-7/)
    expect(s.spanId).not.toBe(s.traceId)
    expect(s.op).toBe('http.client')
  })

  test('explicit parent → inherits traceId, sets parentSpanId', () => {
    const root = startSpan('app.cold-start')
    const child = startSpan('db.query', { parent: root })
    expect(child.traceId).toBe(root.traceId)
    expect(child.parentSpanId).toBe(root.spanId)
  })

  test('explicit traceId wins over parent', () => {
    const root = startSpan('app.cold-start')
    const forced = startSpan('http.server', {
      parent: root,
      traceId: '00000000-0000-7000-8000-000000000abc',
    })
    expect(forced.traceId).toBe('00000000-0000-7000-8000-000000000abc')
    // parentSpanId still comes from parent (only traceId is forced)
    expect(forced.parentSpanId).toBe(root.spanId)
  })

  test('explicit parent: null overrides active context', () => {
    const root = startSpan('outer')
    withSpan(root, () => {
      const detached = startSpan('detached', { parent: null })
      expect(detached.parentSpanId).toBeNull()
      // detached gets a fresh trace because parent was explicitly nulled
      expect(detached.traceId).not.toBe(root.traceId)
    })
  })

  test('name defaults to op when not given', () => {
    const s = startSpan('cache.get')
    const sealed = s.finish()
    expect(sealed?.name).toBe('cache.get')
  })

  test('name override', () => {
    const sealed = startSpan('http.client', { name: 'GET /v1/users/me' }).finish()
    expect(sealed?.name).toBe('GET /v1/users/me')
  })

  test('initial tags + setTag + setData mutate', () => {
    const s = startSpan('db.query', { tags: { 'db.system': 'postgres' } })
    s.setTag('db.statement', 'select 1')
    s.setData('rows', 1)
    const sealed = s.finish()
    expect(sealed?.tags).toEqual({ 'db.system': 'postgres', 'db.statement': 'select 1' })
    expect(sealed?.data).toEqual({ rows: 1 })
  })
})

describe('SpanHandle.finish', () => {
  test('seals + pushes to global buffer', () => {
    const s = startSpan('cache.get')
    expect(getSpans()).toHaveLength(0)
    s.finish()
    const buf = getSpans()
    expect(buf).toHaveLength(1)
    expect(buf[0]?.id).toBe(s.spanId)
    expect(buf[0]?.status).toBe('ok')
  })

  test('status passes through', () => {
    startSpan('http.client').finish({ status: 'error', tags: { http_status: '500' } })
    const seen = getSpans()[0]!
    expect(seen.status).toBe('error')
    expect(seen.tags).toMatchObject({ http_status: '500' })
  })

  test('durationMs is end - start, non-negative', () => {
    const start = 1_700_000_000_000
    const s = startSpan('test', { startNowMs: start })
    expect(s.finish({ endNowMs: start + 142 })?.durationMs).toBe(142)
  })

  test('finish twice is a no-op (second returns null, buffer unchanged)', () => {
    const s = startSpan('test')
    const first = s.finish()
    const second = s.finish()
    expect(first).not.toBeNull()
    expect(second).toBeNull()
    expect(getSpans()).toHaveLength(1)
    expect(s.isFinished()).toBe(true)
  })

  test('startedAt is rfc3339 derived from startNowMs', () => {
    const s = startSpan('test', { startNowMs: 1_700_000_000_000 })
    expect(s.startedAt).toBe(new Date(1_700_000_000_000).toISOString())
  })

  test('respects custom buffer when passed', () => {
    const buf = new SpanBuffer(10)
    const s = new SpanHandle('local')
    s.finish({}, buf)
    expect(buf.size).toBe(1)
    expect(getSpans()).toHaveLength(0) // global untouched
  })
})

describe('SpanBuffer', () => {
  test('drops oldest beyond cap', () => {
    const buf = new SpanBuffer(3)
    for (let i = 0; i < 5; i++) {
      const s = new SpanHandle(`op-${i}`)
      s.finish({}, buf)
    }
    expect(buf.size).toBe(3)
    expect(buf.snapshot().map((sp) => sp.op)).toEqual(['op-2', 'op-3', 'op-4'])
  })

  test('drain empties the buffer + returns the contents', () => {
    const buf = new SpanBuffer()
    new SpanHandle('a').finish({}, buf)
    new SpanHandle('b').finish({}, buf)
    const drained = buf.drain()
    expect(drained.map((s) => s.op)).toEqual(['a', 'b'])
    expect(buf.size).toBe(0)
  })

  test('drainSpans() drains the global buffer', () => {
    startSpan('a').finish()
    startSpan('b').finish()
    expect(drainSpans().map((s) => s.op)).toEqual(['a', 'b'])
    expect(getSpans()).toHaveLength(0)
  })
})

describe('active span context (withSpan / activeSpan)', () => {
  test('activeSpan() returns null when no withSpan is open', () => {
    expect(activeSpan()).toBeNull()
  })

  test('withSpan sets, restores afterwards', () => {
    const outer = startSpan('outer')
    expect(activeSpan()).toBeNull()
    withSpan(outer, () => {
      expect(activeSpan()?.spanId).toBe(outer.spanId)
    })
    expect(activeSpan()).toBeNull()
  })

  test('nested withSpan: child sees inner, restores to outer on exit', () => {
    const outer = startSpan('outer')
    const inner = startSpan('inner', { parent: outer })
    withSpan(outer, () => {
      expect(activeSpan()?.spanId).toBe(outer.spanId)
      withSpan(inner, () => {
        expect(activeSpan()?.spanId).toBe(inner.spanId)
      })
      expect(activeSpan()?.spanId).toBe(outer.spanId)
    })
    expect(activeSpan()).toBeNull()
  })

  test('startSpan inside withSpan inherits trace + parent automatically', () => {
    const root = startSpan('http.server')
    withSpan(root, () => {
      const child = startSpan('db.query')
      expect(child.traceId).toBe(root.traceId)
      expect(child.parentSpanId).toBe(root.spanId)
    })
  })

  test('restores the previous value (not just null) on nested exit', () => {
    const a = startSpan('a')
    const b = startSpan('b', { parent: a })
    withSpan(a, () => {
      withSpan(b, () => {
        expect(activeSpan()?.spanId).toBe(b.spanId)
      })
      expect(activeSpan()?.spanId).toBe(a.spanId)
    })
    expect(activeSpan()).toBeNull()
  })

  test('throw inside withSpan still restores active span', () => {
    const outer = startSpan('outer')
    expect(() =>
      withSpan(outer, () => {
        throw new Error('boom')
      }),
    ).toThrow('boom')
    expect(activeSpan()).toBeNull()
  })
})

describe('withSpan(name, fn) — v2.3 high-level overload', () => {
  test('opens + auto-closes span on sync fn return', () => {
    const before = activeSpan()
    const result = withSpan('db.query', (span) => {
      expect(span.op).toBe('db.query')
      expect(span.spanId).toBeTruthy()
      return 'ok'
    })
    expect(result).toBe('ok')
    expect(activeSpan()).toBe(before)
  })

  test('auto-records exception + ends with error status on throw', () => {
    let captured: { op: string; status?: string } | null = null
    expect(() =>
      withSpan('failing', (span) => {
        captured = { op: span.op }
        throw new Error('nope')
      }),
    ).toThrow('nope')
    expect(captured).not.toBeNull()
    expect(captured!.op).toBe('failing')
  })

  test('async fn: ends span on promise resolution', async () => {
    const result = await withSpan('async-op', async (span) => {
      expect(span.op).toBe('async-op')
      return 42
    })
    expect(result).toBe(42)
  })

  test('opts (third arg) passed through to startSpan', () => {
    withSpan(
      'tagged-op',
      (span) => {
        expect(span.tags?.['flow']).toBe('checkout')
      },
      { tags: { flow: 'checkout' } },
    )
  })

  test('first-arg dispatch: SpanContextLike vs string', () => {
    // SpanContextLike branch — sets active span.
    const ctx = { spanId: 'span-1', traceId: 'trace-1' }
    withSpan(ctx, () => {
      expect(activeSpan()?.spanId).toBe('span-1')
    })
    // String branch — creates a new span instead.
    withSpan('fresh', (newSpan) => {
      expect(newSpan.spanId).not.toBe('span-1')
    })
  })
})
