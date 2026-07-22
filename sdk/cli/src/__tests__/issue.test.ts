import { afterEach, beforeEach, describe, expect, test } from 'bun:test'

import { formatIssueLine, issueList, issuePatch } from '../issue.js'

const cfg = {
  apiUrl: 'https://api.example.com/',
  projectId: 'proj-1',
  token: 'sk_test',
}

const sample = {
  errorType: 'TypeError',
  eventCount: 17,
  id: 'issue-1',
  lastSeen: '2026-05-13T00:00:00Z',
  messageSample: 'undefined is not an object',
  status: 'active' as const,
}

const origFetch = globalThis.fetch
let calls: { body: string | undefined; headers: Headers; method: string; url: string }[]
beforeEach(() => {
  calls = []
})
afterEach(() => {
  globalThis.fetch = origFetch
})

function mockFetch(respBody: string, init: ResponseInit = { status: 200 }): void {
  globalThis.fetch = (async (url: Request | string | URL, opts?: RequestInit) => {
    calls.push({
      body: typeof opts?.body === 'string' ? opts.body : undefined,
      headers: new Headers(opts?.headers),
      method: opts?.method ?? 'GET',
      url: String(url),
    })
    return new Response(respBody, {
      headers: { 'content-type': 'application/json' },
      ...init,
    })
  }) as typeof fetch
}

describe('issueList', () => {
  test('GETs /admin/api/projects/<id>/issues with query params + Bearer', async () => {
    mockFetch(JSON.stringify([sample]))
    const rows = await issueList({ config: cfg, limit: 20, status: 'active' })
    expect(calls).toHaveLength(1)
    expect(calls[0]?.method).toBe('GET')
    expect(calls[0]?.url).toBe(
      'https://api.example.com/admin/api/projects/proj-1/issues?status=active&limit=20',
    )
    expect(calls[0]?.headers.get('authorization')).toBe('Bearer sk_test')
    expect(rows).toHaveLength(1)
    expect(rows[0]?.id).toBe('issue-1')
  })

  test('omits empty query params', async () => {
    mockFetch(JSON.stringify([]))
    await issueList({ config: cfg })
    expect(calls[0]?.url).toBe('https://api.example.com/admin/api/projects/proj-1/issues')
  })

  test('throws with server detail on non-2xx', async () => {
    mockFetch('forbidden', { status: 403, statusText: 'Forbidden' })
    await expect(issueList({ config: cfg })).rejects.toThrow('403 Forbidden — forbidden')
  })
})

describe('issuePatch', () => {
  test('PATCHes /issues/<id> with the body', async () => {
    mockFetch(JSON.stringify({ ...sample, status: 'resolved' }))
    const updated = await issuePatch(cfg, 'issue-1', {
      resolvedInRelease: 'app@1.2.4+457',
      status: 'resolved',
    })
    expect(calls[0]?.method).toBe('PATCH')
    expect(calls[0]?.url).toBe('https://api.example.com/admin/api/projects/proj-1/issues/issue-1')
    expect(JSON.parse(calls[0]?.body ?? '{}')).toEqual({
      resolvedInRelease: 'app@1.2.4+457',
      status: 'resolved',
    })
    expect(updated.status).toBe('resolved')
  })
})

describe('formatIssueLine', () => {
  test('renders id + status + title + event count on one line', () => {
    const line = formatIssueLine(sample)
    expect(line).toContain('issue-1')
    expect(line).toContain('active')
    expect(line).toContain('TypeError: undefined is not an object')
    expect(line).toContain('17×')
  })
})
