// v2.8 — Web Push registration on top of the Sentori server's
// `/v1/push/*` API.
//
// `registerWeb` walks the browser permission → Service Worker →
// PushSubscription → server-register flow and returns the
// `ipt_<uuid>` handle the server uses as a stable device id. The
// caller (host app) decides WHEN to invoke this — Sentori never
// triggers a permission prompt on its own.
//
// The native token persisted server-side is `JSON.stringify(subscription)`
// — the whole `{ endpoint, keys: { p256dh, auth } }` blob. The Web
// Push provider on the server decodes this to drive RFC 8291
// payload encryption.

import { logger } from '@goliapkg/sentori-core'

import { getConfig } from './config.js'

const STORAGE_KEY = 'sentori.push.ipt'

export type RegisterWebOptions = {
  /// URL of the Service Worker script to register. Defaults to
  /// '/sentori-sw.js' — the recipe ships a template the host drops
  /// in at the site root.
  serviceWorkerUrl?: string
  /// Project's VAPID public key, base64url-encoded uncompressed
  /// P-256 (65 bytes). Match the value uploaded to the project's
  /// push_credentials row.
  vapidPublicKey: string
  /// Identity link hash for "push to user X" routing. Mirrors v2.3
  /// identity hashing — pass `hashIdentities({ email: '...' }).email`.
  linkHash?: string
  /// Host metadata to attach to the device_tokens row. Useful for
  /// dashboard filtering ("Chrome 127 devices on Mac"). Optional.
  metadata?: Record<string, unknown>
  /// Foreground notification callback. Fired when the Service
  /// Worker posts a message back to the page with the parsed push
  /// payload. The SW template forwards every push it receives.
  onMessage?: (payload: { title?: string; body?: string; data?: unknown }) => void
  /// User-tapped-a-notification callback. SW posts on
  /// `notificationclick`.
  onTap?: (data: unknown) => void
  /// Fired on any failure in the registration flow. The promise
  /// also rejects; the callback is convenience for callers that
  /// want to log without `.catch()`.
  onError?: (err: Error) => void
}

export type RegisterWebResult = {
  /// Stable device handle (`ipt_<uuid>`). Cached in localStorage so
  /// subsequent `registerWeb` calls return the same handle without
  /// re-subscribing.
  ipt: string
}

/**
 * Register the current browser tab for Web Push and return the
 * resulting `ipt_*` handle. Opt-in: caller invokes when ready.
 *
 * Steps:
 *  1. Permission prompt via `Notification.requestPermission()`.
 *  2. Register the Service Worker (idempotent — reuses an existing
 *     registration with the same scope if present).
 *  3. Subscribe via `pushManager.subscribe()` with the project's
 *     VAPID public key.
 *  4. POST the subscription JSON to `/v1/push/tokens`.
 *  5. Stash the `ipt_*` handle in localStorage + return it.
 *
 * On any rejection the promise rejects with a tagged Error.
 */
export async function registerWeb(opts: RegisterWebOptions): Promise<RegisterWebResult> {
  try {
    assertBrowserApi()
    const cfg = getConfig()
    if (!cfg) throw new Error('sentori not initialised — call initSentori() first')
    const swUrl = opts.serviceWorkerUrl ?? '/sentori-sw.js'

    const permission = await Notification.requestPermission()
    if (permission !== 'granted') {
      throw new Error(`Notification permission '${permission}'; cannot subscribe`)
    }

    const reg = await navigator.serviceWorker.register(swUrl)
    await navigator.serviceWorker.ready

    // pushManager.subscribe is idempotent for the same VAPID key
    // pair — returns the existing subscription if already subscribed.
    const subscription = await reg.pushManager.subscribe({
      userVisibleOnly: true,
      applicationServerKey: urlBase64ToBuffer(opts.vapidPublicKey),
    })

    const ipt = await registerWithServer(cfg, subscription, opts)
    cacheIpt(ipt)

    // Wire SW → page message channel for foreground delivery.
    bindServiceWorkerListener(opts.onMessage, opts.onTap)

    return { ipt }
  } catch (e) {
    const err = e instanceof Error ? e : new Error(String(e))
    logger.warn('sentori.push.registerWeb failed:', err.message)
    opts.onError?.(err)
    throw err
  }
}

/**
 * Revoke the cached `ipt_*` handle (DELETE /v1/push/tokens/{ipt})
 * + unsubscribe locally. Idempotent — repeat calls are no-ops.
 *
 * Does not unregister the Service Worker; another host app feature
 * might rely on it. Customers who own the SW exclusively can
 * `navigator.serviceWorker.getRegistration().then(r => r?.unregister())`
 * after this call.
 */
export async function unregisterWeb(): Promise<void> {
  const cfg = getConfig()
  const ipt = readCachedIpt()
  if (cfg && ipt) {
    try {
      await fetch(joinUrl(cfg.ingestUrl, `/v1/push/tokens/${ipt}`), {
        method: 'DELETE',
        headers: { authorization: `Bearer ${cfg.token}` },
        keepalive: true,
      })
    } catch (e) {
      logger.warn('sentori.push.unregisterWeb: server delete failed', e)
    }
  }
  if (typeof navigator !== 'undefined' && navigator.serviceWorker) {
    try {
      const reg = await navigator.serviceWorker.getRegistration()
      const sub = await reg?.pushManager.getSubscription()
      await sub?.unsubscribe()
    } catch (e) {
      logger.warn('sentori.push.unregisterWeb: local unsubscribe failed', e)
    }
  }
  clearCachedIpt()
}

/// Read the last registered `ipt_*` handle from localStorage.
/// Useful for hosts that want to short-circuit re-registration on
/// page load.
export function readCachedIpt(): string | null {
  try {
    return typeof localStorage !== 'undefined' ? localStorage.getItem(STORAGE_KEY) : null
  } catch {
    return null
  }
}

function cacheIpt(ipt: string): void {
  try {
    if (typeof localStorage !== 'undefined') localStorage.setItem(STORAGE_KEY, ipt)
  } catch {
    /* private mode / quota — non-fatal */
  }
}

function clearCachedIpt(): void {
  try {
    if (typeof localStorage !== 'undefined') localStorage.removeItem(STORAGE_KEY)
  } catch {
    /* non-fatal */
  }
}

function assertBrowserApi(): void {
  if (typeof window === 'undefined') {
    throw new Error('sentori.push.registerWeb requires a browser environment')
  }
  if (!('Notification' in window)) {
    throw new Error('Notification API not available in this browser')
  }
  if (!('serviceWorker' in navigator)) {
    throw new Error('Service Worker API not available in this browser')
  }
  if (!('PushManager' in window)) {
    throw new Error('PushManager not available in this browser')
  }
}

type Cfg = { ingestUrl: string; token: string }

async function registerWithServer(
  cfg: Cfg,
  subscription: PushSubscription,
  opts: RegisterWebOptions,
): Promise<string> {
  const native = JSON.stringify(subscription.toJSON())
  const body = {
    provider: 'webpush',
    nativeToken: native,
    linkHash: opts.linkHash,
    metadata: { ...defaultMetadata(), ...(opts.metadata ?? {}) },
  }
  const res = await fetch(joinUrl(cfg.ingestUrl, '/v1/push/tokens'), {
    method: 'POST',
    headers: {
      authorization: `Bearer ${cfg.token}`,
      'content-type': 'application/json',
    },
    body: JSON.stringify(body),
  })
  if (!res.ok) {
    throw new Error(`/v1/push/tokens HTTP ${res.status}`)
  }
  const json = (await res.json()) as { id?: string }
  if (typeof json.id !== 'string' || !json.id.startsWith('ipt_')) {
    throw new Error('server did not return an ipt_* handle')
  }
  return json.id
}

function defaultMetadata(): Record<string, unknown> {
  if (typeof navigator === 'undefined') return {}
  return {
    userAgent: navigator.userAgent,
    language: navigator.language,
    platform: (navigator as { platform?: string }).platform,
  }
}

function joinUrl(base: string, path: string): string {
  return `${base.replace(/\/+$/, '')}${path}`
}

/**
 * Convert a base64url-encoded VAPID public key into the ArrayBuffer
 * shape `pushManager.subscribe({ applicationServerKey })` accepts.
 * We hand back a `Uint8Array` whose underlying buffer is a fresh
 * `ArrayBuffer` (not `SharedArrayBuffer`) so the strict DOM lib
 * typings line up.
 */
function urlBase64ToBuffer(b64url: string): ArrayBuffer {
  const padding = '='.repeat((4 - (b64url.length % 4)) % 4)
  const base64 = (b64url + padding).replace(/-/g, '+').replace(/_/g, '/')
  const raw =
    typeof atob !== 'undefined'
      ? atob(base64)
      : Buffer.from(base64, 'base64').toString('binary')
  const buf = new ArrayBuffer(raw.length)
  const view = new Uint8Array(buf)
  for (let i = 0; i < raw.length; i++) view[i] = raw.charCodeAt(i)
  return buf
}

type SwForegroundMessage = {
  type: 'sentori.push.message' | 'sentori.push.tap'
  payload: { title?: string; body?: string; data?: unknown }
}

function bindServiceWorkerListener(
  onMessage: RegisterWebOptions['onMessage'],
  onTap: RegisterWebOptions['onTap'],
): void {
  if (!onMessage && !onTap) return
  if (typeof navigator === 'undefined' || !navigator.serviceWorker) return
  navigator.serviceWorker.addEventListener('message', (event: MessageEvent) => {
    const data = event.data as SwForegroundMessage | undefined
    if (!data || !data.type) return
    if (data.type === 'sentori.push.message' && onMessage) onMessage(data.payload)
    if (data.type === 'sentori.push.tap' && onTap) onTap(data.payload?.data)
  })
}
