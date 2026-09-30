// The mini-program binding for the core transport.
//
// Two things here are not like a browser: there is no `fetch`, and
// storage is synchronous with a hard size limit.
//
// `wx.request` is callback-based, so it is wrapped into something
// `fetch`-shaped. The wrapper returns a `Response`-like object with
// only what the transport reads — `status` and `json()` — because
// building a real `Response` would need a polyfill this package has no
// reason to carry.

import { createTransport, type TransportStorage } from '@goliapkg/sentori-core'
import type { AssertStat, SessionPing, WireEvent } from '@goliapkg/sentori-core'

import { getConfig } from './config.js'
import { getWx } from './wx.js'

// Pinned to package.json by a test — bump both together.
export const SDK_VERSION = '1.0.3'

/**
 * `wx.setStorageSync` caps a single key at 1 MB and the whole store at
 * 10 MB, and going over throws. The offline queue must never be the
 * reason a mini program cannot save its own state, so what it writes
 * is capped well under the per-key limit and the write is guarded: a
 * failure means the batch is counted as dropped, which the console
 * shows, rather than an exception on a path the host cannot see.
 */
const MAX_STORED_BYTES = 256 * 1024

const storage = async (): Promise<null | TransportStorage> => {
  const wx = getWx()
  if (!wx) return null
  return {
    getItem: async (k) => {
      try {
        const v = wx.getStorageSync(k)
        return typeof v === 'string' && v.length > 0 ? v : null
      } catch {
        return null
      }
    },
    setItem: async (k, v) => {
      try {
        // Measured in bytes, not characters: the limit is on bytes and
        // a queue of Chinese error messages is three bytes a character,
        // so a character count would let it through at three times the
        // size.
        if (byteLength(v) > MAX_STORED_BYTES) return
        wx.setStorageSync(k, v)
      } catch {
        // Quota, or a storage the platform has locked. The core
        // transport counts what it could not keep.
      }
    },
    removeItem: async (k) => {
      try {
        wx.removeStorageSync(k)
      } catch {
        // Nothing to do and nothing worth telling the host.
      }
    },
  }
}

/** UTF-8 length without `TextEncoder`, which the base library does not
 *  reliably have. */
export const byteLength = (s: string): number => {
  let n = 0
  for (let i = 0; i < s.length; i += 1) {
    const c = s.charCodeAt(i)
    if (c < 0x80) n += 1
    else if (c < 0x800) n += 2
    else if (c >= 0xd800 && c <= 0xdbff) {
      n += 4
      i += 1
    } else n += 3
  }
  return n
}

/** `wx.request` as something the core transport can call. */
const wxFetch = (async (input: unknown, init?: { method?: string; headers?: Record<string, string>; body?: string }) => {
  const wx = getWx()
  if (!wx) throw new Error('no wx')
  return await new Promise((resolve, reject) => {
    wx.request({
      url: String(input),
      method: init?.method ?? 'GET',
      header: init?.headers,
      data: init?.body,
      success: (res) => {
        resolve({
          status: res.statusCode,
          // The base library parses JSON for us when the response
          // says so, and hands back a string when it does not. Both
          // shapes reach here, so both are handled — an unparsed
          // string that gets `JSON.parse`d a second time throws, and
          // the transport reads a thrown body as "cannot tell", which
          // would lose every refusal the server reports.
          json: async () =>
            typeof res.data === 'string' ? safeParse(res.data) : (res.data ?? {}),
        })
      },
      fail: (err) => reject(new Error(err.errMsg ?? 'wx.request failed')),
    })
  })
}) as unknown as typeof fetch

const safeParse = (s: string): unknown => {
  try {
    return JSON.parse(s)
  } catch {
    return {}
  }
}

const transport = createTransport({
  config: () => {
    const c = getConfig()
    if (!c) return null
    return { ingestUrl: c.ingestUrl, token: c.token, backendHealthUrl: c.backendHealthUrl }
  },
  storage,
  sdkLabel: `weapp/${SDK_VERSION}`,
  fetch: wxFetch,
})

export const enqueue = (event: WireEvent): void => transport.enqueue(event)
export const countAssert = (name: string, ok: boolean, release: string): void =>
  transport.countAssert(name, ok, release)
export const queueSession = (ping: SessionPing): void => transport.queueSession(ping)
export const startTransport = (): void => transport.start()
export const flush = (): Promise<void> => transport.flush()
export const drainOfflineQueue = (): Promise<void> => transport.drainOfflineQueue()

export const __resetForTests = (): void => transport.reset()
export const __peekQueue = (): readonly WireEvent[] => transport.peekQueue()
export const __peekDropped = (): number => transport.peekDropped()
export const __peekAssertStats = (): readonly AssertStat[] => transport.peekAssertStats()
