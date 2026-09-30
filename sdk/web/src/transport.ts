// The browser binding for the core transport.
//
// Queueing, backoff, refusal handling and the offline queue all live
// in `@goliapkg/sentori-core`. What is browser-specific is where the
// offline queue goes and what this build calls itself on the wire.

import { createTransport, type TransportStorage } from '@goliapkg/sentori-core'
import type { AssertStat, AttachmentKind, SessionPing, WireEvent } from '@goliapkg/sentori-core'

import { getConfig } from './config.js'

// Pinned to package.json by a test — bump both together.
export const SDK_VERSION = '1.0.3'

/**
 * `localStorage`, wrapped so it cannot take the page down.
 *
 * It throws rather than returning null in two ordinary situations: a
 * Safari private window, and any page where the user has blocked site
 * data. Both are a browser the SDK must keep working in, so every
 * access is guarded and a failure means "no storage", not "no SDK".
 *
 * Synchronous under an async signature because the core transport
 * cannot assume either — a mini-program's storage is synchronous and
 * React Native's is not.
 */
const storage = async (): Promise<null | TransportStorage> => {
  try {
    if (typeof localStorage === 'undefined') return null
    // Prove it works rather than assume it: in a private window the
    // object exists and `setItem` is what throws.
    const probe = '@sentori/probe'
    localStorage.setItem(probe, '1')
    localStorage.removeItem(probe)
  } catch {
    return null
  }
  return {
    getItem: async (k) => {
      try {
        return localStorage.getItem(k)
      } catch {
        return null
      }
    },
    setItem: async (k, v) => {
      try {
        localStorage.setItem(k, v)
      } catch {
        // Quota, most likely. The core transport counts what it could
        // not keep; throwing from here would lose the count too.
      }
    },
    removeItem: async (k) => {
      try {
        localStorage.removeItem(k)
      } catch {
        // Nothing to do, and nothing worth telling the host.
      }
    },
  }
}

const transport = createTransport({
  config: () => {
    const c = getConfig()
    if (!c) return null
    return { ingestUrl: c.ingestUrl, token: c.token, backendHealthUrl: c.backendHealthUrl }
  },
  storage,
  sdkLabel: `web/${SDK_VERSION}`,
  // Read at call time so a test can replace the global.
  fetch: ((input: Parameters<typeof fetch>[0], init?: Parameters<typeof fetch>[1]) =>
    globalThis.fetch(input, init)) as typeof fetch,
})

export const enqueue = (event: WireEvent): void => transport.enqueue(event)
export const countAssert = (name: string, ok: boolean, release: string): void =>
  transport.countAssert(name, ok, release)
export const queueSession = (ping: SessionPing): void => transport.queueSession(ping)
export const startTransport = (): void => transport.start()
export const flush = (): Promise<void> => transport.flush()
export const queueAttachment = (
  eventId: string,
  kind: AttachmentKind,
  blob: { base64?: string; text?: string; mediaType: string },
): void => transport.queueAttachment(eventId, kind, blob, { source: 'js' })
export const drainOfflineQueue = (): Promise<void> => transport.drainOfflineQueue()

export const __resetForTests = (): void => transport.reset()
export const __peekQueue = (): readonly WireEvent[] => transport.peekQueue()
export const __peekDropped = (): number => transport.peekDropped()
export const __peekAssertStats = (): readonly AssertStat[] => transport.peekAssertStats()
export const __peekSessions = (): readonly SessionPing[] => transport.peekSessions()
