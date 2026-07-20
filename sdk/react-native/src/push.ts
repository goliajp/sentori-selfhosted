// v2.9 — React Native push notification opt-in (iOS in this release).
//
// Mirrors `@goliapkg/sentori-javascript`'s `registerWeb` ergonomics
// so a cross-platform host app reasons about both flows the same way.
//
// Flow:
//   1. `pushRequestPermission()` — OS prompt the first time, or
//      returns the cached decision.
//   2. `pushRegister()` — kicks off
//      `UIApplication.registerForRemoteNotifications`. The token
//      arrives asynchronously via the AppDelegate swizzle and lands
//      in the native buffer.
//   3. Poll `pushDrainState()` at 200 ms ticks for up to 8 s waiting
//      for the token.
//   4. POST `/v1/push/tokens` with
//      `provider: 'apns'`, `env: __DEV__ ? 'sandbox' : 'production'`,
//      `nativeToken: <hex>`, `linkHash?`, `metadata`.
//   5. Cache the returned `ipt_*` handle (AsyncStorage when
//      available, otherwise module-scoped).
//   6. Start a 1 Hz drain loop that fires `onMessage` / `onTap` from
//      buffered events while the app is foreground. Pauses on
//      background, resumes on active, per the perf iron rule.

import { addBreadcrumb, logger } from '@goliapkg/sentori-core'

import { track } from './track.js'
// AppState is RN-only; we treat it dynamically so the SDK keeps
// importing cleanly under Bun / web.
type AppStateModule = {
  currentState: string
  addEventListener: (
    type: 'change',
    listener: (state: string) => void,
  ) => { remove: () => void }
}

import {
  pushDrainState,
  pushGetStatus,
  pushRegister as nativePushRegister,
  pushRequestPermission,
  pushUnregister as nativePushUnregister,
} from './native.js'

const STORAGE_KEY = 'sentori.push.ipt'

let _cachedIpt: null | string = null
let _drainInterval: ReturnType<typeof setInterval> | null = null
let _appStateSubscription: { remove: () => void } | null = null
let _backgrounded = false

let _onMessage: PushRegisterOptions['onMessage'] = undefined
let _onTap: PushRegisterOptions['onTap'] = undefined

// v2.26 — confirmed delivery ack pipeline. msgIds extracted from
// received pushes are queued here and flushed to the server every
// 5 s. Server-side `push_sends.acked_at` flips from NULL to
// wall-clock on first ack. See docs/roadmap/v2.26.md.
const ACK_FLUSH_INTERVAL_MS = 5000
let _ackQueue: string[] = []
let _ackFlushInterval: ReturnType<typeof setInterval> | null = null
let _sessionId: null | string = null

export type PushRegisterOptions = {
  /** Identity-link hash. Pass `hashIdentities({ email }).email` if
   *  the host has run the v2.3 identity flow. Lets the server-side
   *  push routing target a specific user across all their devices. */
  linkHash?: string
  /** Extra metadata to attach to the device_tokens row (e.g. app
   *  version, locale). Optional. */
  metadata?: Record<string, unknown>
  /** Foreground notification arrival. Fires once per notification
   *  the SW or iOS native delegate hands us. */
  onMessage?: (payload: PushNotificationPayload) => void
  /** User tapped a notification. Fires once per tap. */
  onTap?: (data: unknown) => void
  /** Token registration completed — useful when the host wants the
   *  ipt handle in real time without awaiting `register()`. */
  onToken?: (ipt: string) => void
  /** Any failure in the registration flow. The promise also
   *  rejects; this callback is convenience. */
  onError?: (err: Error) => void
  /** Override the timeout when waiting for the native token to
   *  arrive after `registerForRemoteNotifications`. Defaults to
   *  8000 ms; bump on slow networks / TestFlight provisioning
   *  delays. */
  tokenTimeoutMs?: number
}

export type PushRegisterResult = {
  /** Stable device handle (`ipt_<uuid>`). */
  ipt: string
}

export type PushNotificationPayload = {
  id?: string
  title?: string
  body?: string
  subtitle?: string
  category?: string
  userInfo?: Record<string, unknown>
  receivedAt?: number
}

/**
 * Run the iOS push opt-in flow. Returns the cached `ipt_*` handle
 * on subsequent calls when permission is still granted.
 */
export async function register(opts: PushRegisterOptions = {}): Promise<PushRegisterResult> {
  try {
    const cfg = getRuntimeConfig()
    // Bind callbacks up front so the buffer drain inside
    // waitForToken can fire onMessage / onTap for events that arrive
    // alongside or before the device token (e.g. user taps a push
    // received during a previous launch — iOS replays it on
    // delegate attach).
    _onMessage = opts.onMessage
    _onTap = opts.onTap
    const status = await pushRequestPermission()
    if (status !== 'granted' && status !== 'provisional' && status !== 'ephemeral') {
      throw new Error(`Push permission '${status ?? 'unavailable'}'; cannot register`)
    }
    nativePushRegister()
    const token = await waitForToken(opts.tokenTimeoutMs ?? 8000)
    const ipt = await registerWithServer(cfg, token, opts)
    _cachedIpt = ipt
    void persistIpt(ipt)
    opts.onToken?.(ipt)
    bindBufferDrain(opts.onMessage, opts.onTap)
    return { ipt }
  } catch (e) {
    const err = e instanceof Error ? e : new Error(String(e))
    logger.warn('push', 'register failed:', err.message)
    opts.onError?.(err)
    throw err
  }
}

/**
 * Revoke the cached handle (DELETE /v1/push/tokens/{ipt}) +
 * unregister locally. Idempotent — repeat calls are no-ops.
 */
export async function unregister(): Promise<void> {
  const cfg = tryGetRuntimeConfig()
  const ipt = _cachedIpt ?? (await readPersistedIpt())
  if (cfg && ipt) {
    try {
      await fetch(joinUrl(cfg.ingestUrl, `/v1/push/tokens/${ipt}`), {
        method: 'DELETE',
        headers: { authorization: `Bearer ${cfg.token}` },
      })
    } catch (e) {
      logger.warn('push', 'unregister server delete failed', e)
    }
  }
  nativePushUnregister()
  _cachedIpt = null
  void clearPersistedIpt()
  teardownBufferDrain()
}

/** Returns the cached handle without hitting the network. Useful
 *  for skipping a re-register prompt across cold starts. */
export function getCachedIpt(): null | string {
  return _cachedIpt
}

/** Public re-export of the no-prompt status check. */
export { pushGetStatus as getStatus, pushRequestPermission as requestPermission }

// ── helpers ────────────────────────────────────────────────────

type RuntimeConfig = { ingestUrl: string; token: string }

function getRuntimeConfig(): RuntimeConfig {
  const cfg = tryGetRuntimeConfig()
  if (!cfg) {
    throw new Error('sentori is not initialised; call sentori.init() first')
  }
  return cfg
}

function tryGetRuntimeConfig(): RuntimeConfig | null {
  // Dynamic require avoids a circular import — `./init` already
  // depends on `./push` via the top-level barrel re-export.
  try {
    const conf = require('./config.js') as { getConfig?: () => null | RuntimeConfig }
    return conf.getConfig?.() ?? null
  } catch {
    return null
  }
}

async function waitForToken(timeoutMs: number): Promise<string> {
  const start = Date.now()
  while (Date.now() - start < timeoutMs) {
    const state = await pushDrainState()
    if (state.error) {
      throw new Error(`APNs registration failed: ${state.error}`)
    }
    if (state.token) {
      // Push any buffered events that arrived alongside the token
      // straight back into the registered listeners (if any).
      flushBuffered(state.notifications, state.taps)
      return state.token
    }
    flushBuffered(state.notifications, state.taps)
    await new Promise((resolve) => setTimeout(resolve, 200))
  }
  throw new Error(`APNs token not received within ${timeoutMs} ms`)
}

async function registerWithServer(
  cfg: RuntimeConfig,
  nativeToken: string,
  opts: PushRegisterOptions,
): Promise<string> {
  // v2.10 — cross-platform. iOS routes via APNs with a
  // sandbox/production env; Android routes via FCM with no env
  // (FCM is a single host). Default to 'apns' when Platform.OS
  // isn't detectable (e.g. unit tests).
  const platform = detectPlatform()
  const isAndroid = platform === 'android'
  const env = isAndroid
    ? undefined
    : typeof __DEV__ !== 'undefined' && __DEV__
      ? 'sandbox'
      : 'production'
  const body: Record<string, unknown> = {
    provider: isAndroid ? 'fcm' : 'apns',
    nativeToken,
    linkHash: opts.linkHash,
    metadata: opts.metadata ?? {},
  }
  if (env != null) body.env = env
  const res = await fetch(joinUrl(cfg.ingestUrl, '/v1/push/tokens'), {
    method: 'POST',
    headers: {
      authorization: `Bearer ${cfg.token}`,
      'content-type': 'application/json',
    },
    body: JSON.stringify(body),
  })
  if (!res.ok) throw new Error(`/v1/push/tokens HTTP ${res.status}`)
  const json = (await res.json()) as { id?: string }
  if (typeof json.id !== 'string' || !json.id.startsWith('ipt_')) {
    throw new Error('server did not return an ipt_* handle')
  }
  return json.id
}

function bindBufferDrain(
  onMessage?: PushRegisterOptions['onMessage'],
  onTap?: PushRegisterOptions['onTap'],
): void {
  _onMessage = onMessage
  _onTap = onTap
  teardownBufferDrain()
  startAppStateWatch()
  _drainInterval = setInterval(() => {
    if (_backgrounded) return
    void pumpOnce()
  }, 1000)
}

function teardownBufferDrain(): void {
  if (_drainInterval) {
    clearInterval(_drainInterval)
    _drainInterval = null
  }
  _appStateSubscription?.remove()
  _appStateSubscription = null
  if (_ackFlushInterval) {
    clearInterval(_ackFlushInterval)
    _ackFlushInterval = null
  }
  _ackQueue = []
}

async function pumpOnce(): Promise<void> {
  const state = await pushDrainState()
  flushBuffered(state.notifications, state.taps)
}

function flushBuffered(
  notifications: Array<Record<string, unknown>>,
  taps: Array<Record<string, unknown>>,
): void {
  for (const raw of notifications) {
    // v2.26 — Observability link-through (rule #4). If the server
    // injected `_sentori.msgId` in v2.25+, drop a `push` breadcrumb,
    // emit `sentori.push.received` track, and queue the ack.
    autoCorrelate(raw, 'received')
    _onMessage?.(coerceNotification(raw))
  }
  for (const raw of taps) {
    autoCorrelate(raw, 'opened')
    _onTap?.(raw.userInfo ?? raw)
  }
}

/** v2.26 — process one drained notification or tap for downstream
 *  correlation. No-op if the payload didn't carry `_sentori.msgId`
 *  (e.g. older server, or push from a non-Sentori sender). */
function autoCorrelate(
  raw: Record<string, unknown>,
  eventType: 'received' | 'opened',
): void {
  const userInfo = (raw.userInfo as Record<string, unknown> | undefined) ?? raw
  const sentori = (userInfo._sentori as Record<string, unknown> | undefined) ?? undefined
  const msgId = typeof sentori?.msgId === 'string' ? sentori.msgId : undefined
  if (!msgId) return

  const provider = guessProvider(raw)
  const title = typeof raw.title === 'string' ? raw.title : undefined
  const body = typeof raw.body === 'string' ? raw.body : undefined

  // Breadcrumb buffer: O(1) in-memory push. Tag both event types
  // ('received' vs 'opened') so a later captureException shows
  // whether the user actually saw the push.
  addBreadcrumb('push', { body, msgId, opened: eventType === 'opened', provider, title })

  // Track event: reuses the existing SDK event pipeline. Two
  // distinct names so dashboards can separate delivery from open.
  const trackName = eventType === 'opened' ? 'sentori.push.opened' : 'sentori.push.received'
  track(trackName, { msgId, provider })

  // Enqueue ack — batched, see drainAckQueue.
  enqueueAck(msgId)
}

function guessProvider(raw: Record<string, unknown>): string {
  if (typeof raw.provider === 'string') return raw.provider
  // iOS native delegate sets `category`; FCM service sets a top-level
  // `from`. Use either as a heuristic; default 'unknown' rather than
  // crashing the pipeline.
  if (raw.from) return 'fcm'
  if (raw.category) return 'apns'
  return 'unknown'
}

function enqueueAck(msgId: string): void {
  if (_ackQueue.includes(msgId)) return
  _ackQueue.push(msgId)
  if (!_ackFlushInterval) {
    _ackFlushInterval = setInterval(() => {
      void drainAckQueue()
    }, ACK_FLUSH_INTERVAL_MS)
  }
}

async function drainAckQueue(): Promise<void> {
  if (_ackQueue.length === 0) return
  const cfg = tryGetRuntimeConfig()
  if (!cfg) return
  const batch = _ackQueue.splice(0, _ackQueue.length)
  // Fire-and-forget — server records first-ack only; subsequent
  // requests are idempotent. Network failure means we lose that
  // ack, which downgrades correlation precision but never breaks
  // the user flow.
  for (const msgId of batch) {
    try {
      await fetch(joinUrl(cfg.ingestUrl, `/v1/push/sends/${msgId}/ack`), {
        body: JSON.stringify({ eventType: 'received', sessionId: _sessionId }),
        headers: {
          authorization: `Bearer ${cfg.token}`,
          'content-type': 'application/json',
        },
        method: 'POST',
      })
    } catch {
      /* best-effort; ignore */
    }
  }
}

/** v2.26 — set the host's current session id so the next ack carries
 *  it. Useful for v2.27 correlation (push -> session -> events). */
export function setSessionContext(sessionId: null | string): void {
  _sessionId = sessionId
}

function coerceNotification(raw: Record<string, unknown>): PushNotificationPayload {
  return {
    id: raw.id as string | undefined,
    title: raw.title as string | undefined,
    body: raw.body as string | undefined,
    subtitle: raw.subtitle as string | undefined,
    category: raw.category as string | undefined,
    userInfo: raw.userInfo as Record<string, unknown> | undefined,
    receivedAt: raw.receivedAt as number | undefined,
  }
}

function startAppStateWatch(): void {
  if (_appStateSubscription) return
  try {
    const rn = require('react-native') as { AppState?: AppStateModule }
    const AppState = rn.AppState
    if (!AppState) return
    _backgrounded = AppState.currentState === 'background'
    _appStateSubscription = AppState.addEventListener('change', (state: string) => {
      _backgrounded = state === 'background'
    })
  } catch {
    /* react-native unavailable (unit test) */
  }
}

async function persistIpt(ipt: string): Promise<void> {
  const storage = await tryAsyncStorage()
  if (!storage) return
  try {
    await storage.setItem(STORAGE_KEY, ipt)
  } catch (e) {
    logger.warn('push', 'AsyncStorage.setItem failed', e)
  }
}

async function clearPersistedIpt(): Promise<void> {
  const storage = await tryAsyncStorage()
  try {
    await storage?.removeItem(STORAGE_KEY)
  } catch (e) {
    logger.warn('push', 'AsyncStorage.removeItem failed', e)
  }
}

async function readPersistedIpt(): Promise<null | string> {
  const storage = await tryAsyncStorage()
  if (!storage) return null
  try {
    return await storage.getItem(STORAGE_KEY)
  } catch {
    return null
  }
}

type AsyncStorageLike = {
  getItem: (k: string) => Promise<null | string>
  setItem: (k: string, v: string) => Promise<void>
  removeItem: (k: string) => Promise<void>
}

async function tryAsyncStorage(): Promise<AsyncStorageLike | null> {
  try {
    const mod = require('@react-native-async-storage/async-storage') as {
      default?: AsyncStorageLike
    }
    return mod.default ?? null
  } catch {
    return null
  }
}

function joinUrl(base: string, path: string): string {
  return `${base.replace(/\/+$/, '')}${path}`
}

let _platformOverride: 'ios' | 'android' | 'unknown' | null = null

/** Test-only hook to override Platform.OS detection. Production
 *  code paths must not call this. */
export function __setPlatformForTests(p: 'ios' | 'android' | 'unknown' | null): void {
  _platformOverride = p
}

function detectPlatform(): 'ios' | 'android' | 'unknown' {
  if (_platformOverride != null) return _platformOverride
  try {
    const rn = require('react-native') as { Platform?: { OS?: string } }
    const os = rn.Platform?.OS
    if (os === 'ios' || os === 'android') return os
  } catch {
    /* react-native unavailable */
  }
  return 'unknown'
}

declare const __DEV__: boolean | undefined
