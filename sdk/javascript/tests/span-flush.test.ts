import { clearSpans, startSpan } from '@goliapkg/sentori-core'
import { afterEach, beforeEach, describe, expect, test } from 'bun:test'

import { setConfig, __resetForTests as resetConfig } from '../src/config.js'
import { flushSpans } from '../src/transport.js'

type Call = { body: unknown; init: RequestInit | undefined; url: string }

let originalFetch: typeof fetch | undefined
let calls: Call[]

beforeEach(() => {
  originalFetch = globalThis.fetch
  calls = []
  clearSpans()
  resetConfig()
  setConfig({
    environment: 'test',
    ingestUrl: 'https://ingest.example.com/',
    release: 'app@1.0.0+1',
    token: 'st_pk_test',
  })
})
afterEach(() => {
  if (originalFetch) globalThis.fetch = originalFetch
  clearSpans()
  resetConfig()
})

function recordFetch(status = 202): void {
  globalThis.fetch = (async (url: Request | string | URL, init?: RequestInit) => {
    calls.push({
      body: JSON.parse((init?.body as string) ?? '{}'),
      init,
      url: String(url),
    })
    return new Response(null, { status })
  }) as unknown as typeof fetch
}

describe('flushSpans', () => {
  test('POSTs buffered spans to /v1/spans:batch (trailing slash trimmed)', async () => {
    recordFetch()
    startSpan('http.client', { name: 'GET /a' }).finish({ status: 'ok' })
    startSpan('http.client', { name: 'GET /b' }).finish({ status: 'error' })
    await flushSpans()

    expect(calls).toHaveLength(1)
    expect(calls[0]?.url).toBe('https://ingest.example.com/v1/spans:batch')
    const headers = calls[0]?.init?.headers as Record<string, string>
    expect(headers.Authorization).toBe('Bearer st_pk_test')
    const body = calls[0]?.body as { spans: unknown[] }
    expect(body.spans).toHaveLength(2)
    expect(body.spans[0]).toMatchObject({ name: 'GET /a', op: 'http.client', status: 'ok' })
  })

  test('no-op when buffer empty', async () => {
    recordFetch()
    await flushSpans()
    expect(calls).toHaveLength(0)
  })

  test('no-op when not configured', async () => {
    resetConfig()
    recordFetch()
    startSpan('http.client').finish()
    await flushSpans()
    expect(calls).toHaveLength(0)
  })

  test('splits >200 spans into multiple batches', async () => {
    recordFetch()
    for (let i = 0; i < 450; i++) startSpan('http.client', { name: `GET /${i}` }).finish()
    await flushSpans()
    expect(calls.map((c) => (c.body as { spans: unknown[] }).spans.length)).toEqual([200, 200, 50])
  })

  test('drops remaining batches on 5xx without retrying', async () => {
    recordFetch(503)
    for (let i = 0; i < 300; i++) startSpan('http.client').finish()
    await flushSpans()
    // first chunk POSTed, hit 503 → stop (no retry, no second chunk)
    expect(calls).toHaveLength(1)
  })
})
