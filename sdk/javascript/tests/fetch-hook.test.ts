import { clearSpans, drainSpans } from '@goliapkg/sentori-core'
import { afterEach, beforeEach, describe, expect, test } from 'bun:test'

import { setConfig, __resetForTests as resetConfig } from '../src/config.js'
import {
  installFetchInstrumentation,
  toTraceparent,
  uninstallFetchInstrumentation,
} from '../src/hooks/fetch.js'

// We replace globalThis.fetch with a recorder before installing the
// instrumentation. The wrapper takes a reference to the recorder at
// install time; uninstall puts the recorder back.

type FetchCall = { input: string; init: RequestInit | undefined }

function makeRecorder(
  responses: Array<{ status: number } | Error> = [{ status: 200 }],
): { calls: FetchCall[]; fn: typeof fetch } {
  const calls: FetchCall[] = []
  let i = 0
  const fn = (async (input: Request | string | URL, init?: RequestInit) => {
    const url =
      typeof input === 'string'
        ? input
        : input instanceof URL
          ? input.toString()
          : input.url
    calls.push({ input: url, init })
    const r = responses[Math.min(i++, responses.length - 1)]
    if (r instanceof Error) throw r
    return new Response('', { status: r?.status ?? 200 })
  }) as unknown as typeof fetch
  return { calls, fn }
}

let originalFetch: typeof fetch | undefined
beforeEach(() => {
  originalFetch = globalThis.fetch
  clearSpans()
})
afterEach(() => {
  uninstallFetchInstrumentation()
  if (originalFetch) globalThis.fetch = originalFetch
  clearSpans()
  resetConfig()
})

describe('toTraceparent', () => {
  test('strips dashes and truncates spanId to 16 hex chars', () => {
    const tp = toTraceparent(
      '019e2000-0000-7100-8000-000000000099',
      '019e2000-0000-7300-8000-000000000001',
    )
    // version=00, traceId=32 hex (no dashes), spanId=first 16 hex of
    // span-uuid (no dashes), flags=01
    expect(tp).toBe('00-019e200000007100800000000000099-019e200000007300-01'.replace(
      /^00-(.{31})-(.{16})-01$/,
      '00-019e2000000071008000000000000099-019e200000007300-01',
    ))
    expect(tp).toMatch(/^00-[0-9a-f]{32}-[0-9a-f]{16}-01$/)
    expect(tp).toBe('00-019e2000000071008000000000000099-019e200000007300-01')
  })

  test('lowercases hex', () => {
    const tp = toTraceparent('AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE', 'F1F2F3F4-1234-5678-9ABC-DEF012345678')
    expect(tp).toBe('00-aaaaaaaabbbbccccddddeeeeeeeeeeee-f1f2f3f412345678-01')
  })
})

describe('installFetchInstrumentation', () => {
  test('is idempotent — second call no-op', () => {
    const recorder = makeRecorder()
    globalThis.fetch = recorder.fn
    expect(installFetchInstrumentation()).toBe(true)
    const wrapped = globalThis.fetch
    expect(installFetchInstrumentation()).toBe(true)
    expect(globalThis.fetch).toBe(wrapped) // not re-wrapped
  })

  test('uninstall restores original fetch', () => {
    const recorder = makeRecorder()
    globalThis.fetch = recorder.fn
    installFetchInstrumentation()
    expect(globalThis.fetch).not.toBe(recorder.fn)
    uninstallFetchInstrumentation()
    expect(globalThis.fetch).toBe(recorder.fn)
  })
})

describe('wrapped fetch', () => {
  test('emits a span with http.method, http.url, http.status', async () => {
    const recorder = makeRecorder([{ status: 201 }])
    globalThis.fetch = recorder.fn
    installFetchInstrumentation()

    const resp = await fetch('https://api.example.com/v1/users/me', {
      method: 'POST',
    })
    expect(resp.status).toBe(201)

    const spans = drainSpans()
    expect(spans).toHaveLength(1)
    expect(spans[0]?.op).toBe('http.client')
    expect(spans[0]?.name).toBe('POST https://api.example.com/v1/users/me')
    expect(spans[0]?.tags).toMatchObject({
      'http.method': 'POST',
      'http.status': '201',
      'http.url': 'https://api.example.com/v1/users/me',
    })
    expect(spans[0]?.status).toBe('ok')
    expect(spans[0]?.durationMs).toBeGreaterThanOrEqual(0)
  })

  test('injects W3C traceparent header', async () => {
    const recorder = makeRecorder()
    globalThis.fetch = recorder.fn
    installFetchInstrumentation()

    await fetch('https://api.example.com/x')
    expect(recorder.calls).toHaveLength(1)
    const headers = new Headers(recorder.calls[0]?.init?.headers)
    const tp = headers.get('traceparent')
    expect(tp).not.toBeNull()
    expect(tp).toMatch(/^00-[0-9a-f]{32}-[0-9a-f]{16}-01$/)
  })

  test('preserves caller-supplied headers alongside traceparent', async () => {
    const recorder = makeRecorder()
    globalThis.fetch = recorder.fn
    installFetchInstrumentation()

    await fetch('https://api.example.com/x', {
      headers: { Authorization: 'Bearer xyz', 'X-Custom': '1' },
    })
    const headers = new Headers(recorder.calls[0]?.init?.headers)
    expect(headers.get('authorization')).toBe('Bearer xyz')
    expect(headers.get('x-custom')).toBe('1')
    expect(headers.get('traceparent')).toBeTruthy()
  })

  test('4xx/5xx → span.status = "error", http.status tag set', async () => {
    const recorder = makeRecorder([{ status: 503 }])
    globalThis.fetch = recorder.fn
    installFetchInstrumentation()

    await fetch('https://api.example.com/x')
    const sp = drainSpans()[0]!
    expect(sp.status).toBe('error')
    expect(sp.tags['http.status']).toBe('503')
  })

  test('network throw → span.status = "error", error.message tag', async () => {
    const recorder = makeRecorder([new TypeError('NetworkError: offline')])
    globalThis.fetch = recorder.fn
    installFetchInstrumentation()

    await expect(fetch('https://api.example.com/x')).rejects.toThrow('NetworkError')
    const sp = drainSpans()[0]!
    expect(sp.status).toBe('error')
    expect(sp.tags['error.message']).toContain('offline')
  })

  test('AbortError → span.status = "cancelled"', async () => {
    const abortError = Object.assign(new Error('aborted'), { name: 'AbortError' })
    const recorder = makeRecorder([abortError])
    globalThis.fetch = recorder.fn
    installFetchInstrumentation()

    await expect(fetch('https://api.example.com/x')).rejects.toThrow('aborted')
    const sp = drainSpans()[0]!
    expect(sp.status).toBe('cancelled')
  })

  test('span name normalizes id-like path segments (full url stays in tag)', async () => {
    const recorder = makeRecorder()
    globalThis.fetch = recorder.fn
    installFetchInstrumentation()

    await fetch('https://api.example.com/devices/69ef2dc5c11ea3820b7cfd1d?token=secret', {
      method: 'GET',
    })
    const sp = drainSpans()[0]!
    expect(sp.name).toBe('GET https://api.example.com/devices/{id}')
    expect(sp.tags['http.url']).toBe('https://api.example.com/devices/69ef2dc5c11ea3820b7cfd1d?token=secret')
  })

  test('does not trace requests to the configured ingest URL', async () => {
    setConfig({
      environment: 'test',
      ingestUrl: 'https://ingest.example.com',
      release: 'app@1.0.0+1',
      token: 'st_pk_test',
    })
    const recorder = makeRecorder()
    globalThis.fetch = recorder.fn
    installFetchInstrumentation()

    await fetch('https://ingest.example.com/v1/spans:batch', { method: 'POST' })
    // request still went through, but no span and no traceparent header
    expect(recorder.calls).toHaveLength(1)
    expect(new Headers(recorder.calls[0]?.init?.headers).get('traceparent')).toBeNull()
    expect(drainSpans()).toHaveLength(0)

    // a non-ingest request is still instrumented
    await fetch('https://api.example.com/x')
    expect(drainSpans()).toHaveLength(1)
  })

  test('accepts URL instance + Request input shapes', async () => {
    const recorder = makeRecorder()
    globalThis.fetch = recorder.fn
    installFetchInstrumentation()

    await fetch(new URL('https://api.example.com/u'), { method: 'PATCH' })
    const req = new Request('https://api.example.com/r', { method: 'DELETE' })
    await fetch(req)

    const spans = drainSpans()
    expect(spans[0]?.name).toBe('PATCH https://api.example.com/u')
    expect(spans[1]?.name).toBe('DELETE https://api.example.com/r')
  })
})
