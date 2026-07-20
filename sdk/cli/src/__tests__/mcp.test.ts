import { afterEach, beforeEach, describe, expect, test } from 'bun:test'

import { buildTools } from '../mcp.js'

const ADMIN = {
  apiUrl: 'https://api.example.com/',
  projectId: 'proj-uuid',
  token: 'sk_test',
}

const origFetch = globalThis.fetch
let calls: { body: string | null; headers: Headers; method: string; url: string }[]

beforeEach(() => {
  calls = []
})
afterEach(() => {
  globalThis.fetch = origFetch
})

function mockFetch(status = 200, body: unknown = {}): void {
  globalThis.fetch = (async (url: Request | string | URL, init?: RequestInit) => {
    const b = init?.body
    calls.push({
      body: typeof b === 'string' ? b : null,
      headers: new Headers(init?.headers),
      method: init?.method ?? 'GET',
      url: String(url),
    })
    return new Response(status === 204 ? '' : JSON.stringify(body), { status })
  }) as typeof fetch
}

describe('buildTools', () => {
  test('returns the expected set of tools with required input schema fields', () => {
    const tools = buildTools()
    const names = new Set(tools.map((t) => t.name))
    for (const expected of [
      'sentori_issue_list',
      'sentori_issue_get',
      'sentori_issue_comment',
      'sentori_issue_transition',
      'sentori_issue_assign',
      'sentori_issue_set_priority',
      'sentori_issue_set_labels',
      'sentori_issue_watch',
    ]) {
      expect(names.has(expected)).toBe(true)
    }
    for (const t of tools) {
      // Every tool advertises a description + an input schema.
      expect(typeof t.description).toBe('string')
      expect(t.inputSchema).toBeDefined()
      expect((t.inputSchema as { type?: string }).type).toBe('object')
    }
  })
})

describe('tool handlers proxy to admin API', () => {
  test('sentori_issue_list calls GET .../issues with filters', async () => {
    const tools = buildTools()
    const list = tools.find((t) => t.name === 'sentori_issue_list')!
    mockFetch(200, [])
    await list.handler(
      { limit: 25, priority: 'p0', projectId: 'proj-uuid', status: 'active' },
      ADMIN,
    )
    expect(calls.length).toBe(1)
    expect(calls[0]!.url).toContain('/admin/api/projects/proj-uuid/issues?')
    expect(calls[0]!.url).toContain('status=active')
    expect(calls[0]!.url).toContain('priority=p0')
    expect(calls[0]!.url).toContain('limit=25')
    expect(calls[0]!.method).toBe('GET')
    expect(calls[0]!.headers.get('authorization')).toBe('Bearer sk_test')
  })

  test('sentori_issue_get fetches issue + activity in parallel', async () => {
    const tools = buildTools()
    const get = tools.find((t) => t.name === 'sentori_issue_get')!
    mockFetch(200, { stub: true })
    await get.handler({ issueId: 'iss-uuid', projectId: 'proj-uuid' }, ADMIN)
    expect(calls.length).toBe(2)
    const paths = calls.map((c) => c.url).sort()
    expect(paths[0]).toContain('/admin/api/projects/proj-uuid/issues/iss-uuid')
    expect(paths[1]).toContain('/admin/api/projects/proj-uuid/issues/iss-uuid/activity')
  })

  test('sentori_issue_transition PATCHes status', async () => {
    const tools = buildTools()
    const t = tools.find((x) => x.name === 'sentori_issue_transition')!
    mockFetch(200, { id: 'x' })
    await t.handler({ issueId: 'iss', projectId: 'proj', status: 'resolved' }, ADMIN)
    expect(calls[0]!.method).toBe('PATCH')
    expect(JSON.parse(calls[0]!.body!).status).toBe('resolved')
  })

  test('sentori_issue_set_labels rejects non-string array entries', async () => {
    const tools = buildTools()
    const t = tools.find((x) => x.name === 'sentori_issue_set_labels')!
    await expect(
      t.handler({ issueId: 'iss', labels: ['ok', 42], projectId: 'proj' }, ADMIN),
    ).rejects.toThrow(/each label/)
  })

  test('sentori_issue_watch=true → PUT, watch=false → DELETE', async () => {
    const tools = buildTools()
    const t = tools.find((x) => x.name === 'sentori_issue_watch')!
    mockFetch(204)
    await t.handler({ issueId: 'iss', projectId: 'proj', watch: true }, ADMIN)
    expect(calls[0]!.method).toBe('PUT')
    calls.length = 0
    await t.handler({ issueId: 'iss', projectId: 'proj', watch: false }, ADMIN)
    expect(calls[0]!.method).toBe('DELETE')
  })

  test('handler surfaces a non-2xx as an Error', async () => {
    const tools = buildTools()
    const t = tools.find((x) => x.name === 'sentori_issue_comment')!
    mockFetch(500, { error: 'boom' })
    await expect(
      t.handler({ body: 'hello', issueId: 'iss', projectId: 'proj' }, ADMIN),
    ).rejects.toThrow(/500/)
  })
})
