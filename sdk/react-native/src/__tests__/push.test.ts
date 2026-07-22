import { afterEach, beforeEach, describe, expect, it, mock } from 'bun:test'

// Mock the native bridge layer only — leaves expo-modules-core / react-native
// pristine so other test files in the same bun process aren't affected.

type DrainResult = {
  token?: string
  error?: string
  notifications: Array<Record<string, unknown>>
  taps: Array<Record<string, unknown>>
}

let _drainQueue: DrainResult[] = []
let _permissionAnswer: null | string = null
let _registerInvocations = 0
let _unregisterInvocations = 0

mock.module('../native', () => ({
  pushGetStatus: async () => _permissionAnswer,
  pushRequestPermission: async () => _permissionAnswer ?? 'notDetermined',
  pushRegister: () => {
    _registerInvocations++
  },
  pushUnregister: () => {
    _unregisterInvocations++
  },
  pushDrainState: async () => {
    if (_drainQueue.length === 0) {
      return { notifications: [], taps: [] }
    }
    return _drainQueue.shift()!
  },
}))

// Mock the config getter — push reads ingest URL + bearer from here.
// Stub ALL public exports so other test files that import named
// exports from '../config' don't get a "Export named X not found"
// error from the partial mock (bun mock.module replaces the whole
// module surface, not just the named exports listed).
let _mockConfig: { ingestUrl: string; token: string } | null = null
mock.module('../config', () => ({
  getConfig: () => _mockConfig,
  isInitialized: () => _mockConfig !== null,
  setConfig: (c: typeof _mockConfig) => {
    _mockConfig = c
  },
  __resetForTests: () => {
    _mockConfig = null
  },
}))

import { register, unregister, getCachedIpt, __setPlatformForTests } from '../push'

type FetchCall = { url: string; method: string; body?: string }
let _fetchCalls: FetchCall[] = []
let _fetchResponse: { status: number; body: unknown } = {
  status: 200,
  body: { id: 'ipt_deadbeef' },
}

const originalFetch = globalThis.fetch
beforeEach(() => {
  _fetchCalls = []
  _mockConfig = { ingestUrl: 'https://ingest.test', token: 'st_test' }
  _drainQueue = []
  _permissionAnswer = null
  _registerInvocations = 0
  _unregisterInvocations = 0
  _fetchResponse = { status: 200, body: { id: 'ipt_deadbeef' } }
  globalThis.fetch = (async (
    input: RequestInfo | URL,
    init?: RequestInit,
  ): Promise<Response> => {
    _fetchCalls.push({
      url: typeof input === 'string' ? input : input.toString(),
      method: init?.method ?? 'GET',
      body: typeof init?.body === 'string' ? init.body : undefined,
    })
    return new Response(JSON.stringify(_fetchResponse.body), {
      status: _fetchResponse.status,
    })
  }) as typeof fetch
})

afterEach(() => {
  globalThis.fetch = originalFetch
})

describe('push.register', () => {
  it('rejects cleanly when permission is denied', async () => {
    _permissionAnswer = 'denied'
    await expect(register()).rejects.toThrow(/permission/i)
    expect(_fetchCalls).toHaveLength(0)
  })

  it('rejects when the native token times out', async () => {
    _permissionAnswer = 'granted'
    await expect(register({ tokenTimeoutMs: 50 })).rejects.toThrow(/not received/i)
    expect(_fetchCalls).toHaveLength(0)
  })

  it('POSTs to /v1/push/tokens with the APNs hex token and resolves to ipt', async () => {
    _permissionAnswer = 'granted'
    _drainQueue = [
      { notifications: [], taps: [] },
      { token: '0123abcdef', notifications: [], taps: [] },
    ]
    const result = await register({ linkHash: 'h1' })
    expect(result.ipt).toBe('ipt_deadbeef')
    expect(getCachedIpt()).toBe('ipt_deadbeef')
    expect(_fetchCalls).toHaveLength(1)
    expect(_fetchCalls[0]?.url).toContain('/v1/push/tokens')
    expect(_fetchCalls[0]?.method).toBe('POST')
    const body = JSON.parse(_fetchCalls[0]?.body ?? '{}')
    expect(body.provider).toBe('apns')
    expect(body.nativeToken).toBe('0123abcdef')
    expect(body.linkHash).toBe('h1')
    expect(_registerInvocations).toBe(1)
  })

  it('surfaces server failures', async () => {
    _permissionAnswer = 'granted'
    _drainQueue = [{ token: '00ff', notifications: [], taps: [] }]
    _fetchResponse = { status: 503, body: { error: 'dbNotConfigured' } }
    await expect(register()).rejects.toThrow(/503/)
  })

  it('fires onMessage for buffered notifications surfaced during waitForToken', async () => {
    _permissionAnswer = 'granted'
    _drainQueue = [
      {
        notifications: [
          { id: 'n1', title: 'Hi', body: 'hello', userInfo: { x: 1 } },
        ],
        taps: [],
      },
      { token: 'abcd', notifications: [], taps: [] },
    ]
    const seen: Array<{ title?: string; body?: string }> = []
    await register({ onMessage: (m) => seen.push(m) })
    expect(seen).toHaveLength(1)
    expect(seen[0]?.title).toBe('Hi')
    expect(seen[0]?.body).toBe('hello')
  })
})

describe('push.register — Android (FCM) branch', () => {
  beforeEach(() => __setPlatformForTests('android'))
  afterEach(() => __setPlatformForTests(null))

  it('POSTs with provider:"fcm" and omits env on Android', async () => {
    _permissionAnswer = 'granted'
    _drainQueue = [{ token: 'fcm-reg-token', notifications: [], taps: [] }]
    await register()
    expect(_fetchCalls).toHaveLength(1)
    const body = JSON.parse(_fetchCalls[0]?.body ?? '{}') as Record<string, unknown>
    expect(body.provider).toBe('fcm')
    expect(body.nativeToken).toBe('fcm-reg-token')
    expect('env' in body).toBe(false)
  })
})

describe('push.auto-correlate (v2.26)', () => {
  it('writes a push breadcrumb when notification carries _sentori.msgId', async () => {
    const { clearBreadcrumbs, getBreadcrumbs } = await import('@goliapkg/sentori-core')
    clearBreadcrumbs()
    _permissionAnswer = 'granted'
    _drainQueue = [
      {
        notifications: [
          {
            body: 'a body',
            id: 'n2',
            title: 'A title',
            userInfo: { _sentori: { msgId: 'send_abc123' } },
          },
        ],
        taps: [],
      },
      { notifications: [], taps: [], token: 'abcd' },
    ]
    await register()
    const crumbs = getBreadcrumbs()
    const pushCrumb = crumbs.find((c) => c.type === 'push')
    expect(pushCrumb).toBeDefined()
    expect((pushCrumb?.data as Record<string, unknown>)?.msgId).toBe('send_abc123')
    expect((pushCrumb?.data as Record<string, unknown>)?.title).toBe('A title')
    expect((pushCrumb?.data as Record<string, unknown>)?.opened).toBe(false)
  })

  it('skips breadcrumb when payload has no _sentori.msgId', async () => {
    const { clearBreadcrumbs, getBreadcrumbs } = await import('@goliapkg/sentori-core')
    clearBreadcrumbs()
    _permissionAnswer = 'granted'
    _drainQueue = [
      { notifications: [{ id: 'n3', title: 'No correlation' }], taps: [] },
      { notifications: [], taps: [], token: 'abcd' },
    ]
    await register()
    const crumbs = getBreadcrumbs()
    expect(crumbs.find((c) => c.type === 'push')).toBeUndefined()
  })

  it('marks tap breadcrumb as opened:true', async () => {
    const { clearBreadcrumbs, getBreadcrumbs } = await import('@goliapkg/sentori-core')
    clearBreadcrumbs()
    _permissionAnswer = 'granted'
    _drainQueue = [
      {
        notifications: [],
        taps: [{ userInfo: { _sentori: { msgId: 'send_tap' } } }],
      },
      { notifications: [], taps: [], token: 'abcd' },
    ]
    await register()
    const pushCrumb = getBreadcrumbs().find((c) => c.type === 'push')
    expect(pushCrumb).toBeDefined()
    expect((pushCrumb?.data as Record<string, unknown>)?.opened).toBe(true)
    expect((pushCrumb?.data as Record<string, unknown>)?.msgId).toBe('send_tap')
  })
})

describe('push.unregister', () => {
  it('DELETEs the cached ipt and clears the local state', async () => {
    _permissionAnswer = 'granted'
    _drainQueue = [{ token: 'feedface', notifications: [], taps: [] }]
    await register()
    expect(getCachedIpt()).toBe('ipt_deadbeef')
    _fetchCalls = []

    await unregister()
    expect(_fetchCalls).toHaveLength(1)
    expect(_fetchCalls[0]?.url).toContain('/v1/push/tokens/ipt_deadbeef')
    expect(_fetchCalls[0]?.method).toBe('DELETE')
    expect(getCachedIpt()).toBeNull()
    expect(_unregisterInvocations).toBeGreaterThan(0)
  })
})
