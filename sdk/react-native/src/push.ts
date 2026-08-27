// React Native push notification opt-in. Both platforms: iOS via
// APNs, Android via FCM.
//
// Flow:
//   1. `pushRequestPermission()` — OS prompt the first time, or
//      returns the cached decision.
//   2. `pushRegister()` — on iOS kicks off
//      `UIApplication.registerForRemoteNotifications`; on Android
//      asks FCM for the instance token. Either way the token arrives
//      asynchronously and lands in the native buffer.
//   3. Poll `pushDrainState()` at 200 ms ticks for up to 8 s waiting
//      for the token.
//   4. POST `/v1/push/devices` with `kind: 'apns' | 'fcm'` — the
//      server's field name; sending `provider` here earned a 422 for
//      every registration this SDK ever attempted — plus
//      `nativeToken`, `userKey`, and `env` on iOS only (FCM is a
//      single host and has no sandbox/production split).
//   5. Cache the returned device handle (AsyncStorage when
//      available, otherwise module-scoped).
//   6. Start a 1 Hz drain loop that fires `onMessage` / `onTap` from
//      buffered events while the app is foreground. Pauses on
//      background, resumes on active, per the perf iron rule.

import { logger, pushSignal } from '@goliapkg/sentori-core'

import { currentUserKey, currentUserTraits, onIdentityChange } from './scope.js'

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
/// Which installation this is.
///
/// Minted once and kept, so the server can key the device row on it
/// rather than on the vendor's token. Without one a rotation writes a
/// *new* row under a *new* spToken, and every backend holding the old
/// one is addressing nothing — with nothing to tell it. The native
/// SDKs were given this; this package registers from JS and was not.
const INSTALL_KEY = 'sentori.push.install'

let _cachedIpt: null | string = null
// The last token the vendor handed us, kept so a sign-in can re-send
// the registration without asking the OS for a token again.
let _lastNativeToken: null | string = null
let _lastOptions: PushRegisterOptions = {}
// What the server was last told. A re-registration that would send
// the same thing is not sent: `user()` is a verb an app may call on
// every screen, and one HTTP request per call is not free to a host.
let _lastSentIdentity: string | null = null
let _installId: null | string = null
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
  /** Extra metadata stored on the device row and shown in
   *  Settings ▸ Push (e.g. app version, locale, build channel).
   *
   *  Not an identity mechanism. What makes a device addressable is
   *  `sentori.user()`, whose salted hash goes up as `userKey` —
   *  putting a raw user id in here instead sends the real identity
   *  to the server and still leaves the device unaddressable. */
  metadata?: Record<string, unknown>
  /** Foreground notification arrival. Fires once per notification
   *  the SW or iOS native delegate hands us. */
  onMessage?: (payload: PushNotificationPayload) => void
  /** User tapped a notification. Fires once per tap. */
  onTap?: (data: unknown) => void
  /** Token registration completed — useful when the host wants the
   *  ipt handle in real time without awaiting `register()`. */
  onToken?: (ipt: string) => void
  /** Any failure in the registration flow. Convenience only — the
   *  promise resolves to `{ ok: false, reason }` either way and
   *  never rejects. */
  onError?: (err: Error) => void
  /** Override the timeout when waiting for the native token to
   *  arrive after `registerForRemoteNotifications`. Defaults to
   *  8000 ms; bump on slow networks / TestFlight provisioning
   *  delays. */
  tokenTimeoutMs?: number
}

/** Why a registration did not produce a device handle. Each value is
 *  a different thing for the host to do, which is the only reason to
 *  distinguish them:
 *
 *  - `not-initialised` — `sentori.init()` has not run. A wiring bug.
 *  - `permission-denied` — the user said no. Not an error. Do not
 *    retry on a timer; offer it again from a settings screen.
 *  - `no-transport` — no native push module in this binary (Expo Go,
 *    a simulator without a push entitlement). Nothing to do at
 *    runtime.
 *  - `token-timeout` — the OS never handed back a token inside the
 *    window. Usually provisioning; retrying later is reasonable.
 *  - `server-rejected` — Sentori answered non-2xx. Settings ▸ Push
 *    is where to look.
 */
export type PushRegisterFailure =
  | 'not-initialised'
  | 'no-transport'
  | 'permission-denied'
  | 'server-rejected'
  | 'token-timeout'

/** Registration outcome. `register()` never throws and there is no
 *  `catch` branch to forget: a denied permission is an ordinary
 *  answer, not an exception. An opt-in that throws inside someone's
 *  `useEffect` is precisely the failure this SDK's contract with its
 *  host app is written against. */
export type PushRegisterResult =
  | {
      ok: true
      /** Stable device handle: the `device_tokens` row id, a bare
       *  uuid. Named `ipt` for the handle format this once returned;
       *  the revoke and send routes take the uuid. */
      ipt: string
    }
  | { ok: false; message: string; reason: PushRegisterFailure }

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
 * Run the push opt-in flow on either platform. Safe to call on every
 * launch: the OS returns its cached permission decision without
 * re-prompting, and the server upserts on
 * `(project_id, provider, native_token)`. Use `getCachedIpt()` if you
 * want the handle without a round trip.
 *
 * Never throws. Every way this can fail comes back as
 * `{ ok: false, reason }` — see `PushRegisterFailure` for what each
 * one asks the host to do.
 */
export async function register(opts: PushRegisterOptions = {}): Promise<PushRegisterResult> {
  try {
    const cfg = tryGetRuntimeConfig()
    if (!cfg) {
      return fail(opts, 'not-initialised', 'sentori.init() has not run')
    }
    // Bind callbacks up front so the buffer drain inside
    // waitForToken can fire onMessage / onTap for events that arrive
    // alongside or before the device token (e.g. user taps a push
    // received during a previous launch — iOS replays it on
    // delegate attach).
    _onMessage = opts.onMessage
    _onTap = opts.onTap
    const status = await pushRequestPermission()
    // `null` means there is no native push module in this binary —
    // a different thing from the user declining, and a different
    // thing for the host to do about it. These were one branch and
    // one message until now.
    if (status == null) {
      return fail(opts, 'no-transport', 'no native push module in this build')
    }
    if (status !== 'granted' && status !== 'provisional' && status !== 'ephemeral') {
      return fail(opts, 'permission-denied', `push permission '${status}'`)
    }
    nativePushRegister()
    const token = await waitForToken(opts.tokenTimeoutMs ?? 8000)
    const ipt = await registerWithServer(cfg, token, opts)
    _cachedIpt = ipt
    _lastNativeToken = token
    _lastOptions = opts
    void persistIpt(ipt)
    // The registration reached the server and came back with a handle.
    // What the host does with it afterwards is not our outcome — this
    // used to sit inside the try, so a throwing `onToken` reported a
    // successful registration as `server-rejected`.
    safely('onToken', () => opts.onToken?.(ipt))
    bindBufferDrain(opts.onMessage, opts.onTap)
    // From here on, signing in or out updates the row by itself.
    onIdentityChange(reRegisterAfterIdentityChange)
    return { ok: true, ipt }
  } catch (e) {
    const err = e instanceof Error ? e : new Error(String(e))
    const reason = e instanceof PushRegisterError ? e.reason : 'server-rejected'
    return fail(opts, reason, err.message)
  }
}

/** One exit for every failure: warn once, tell `onError` if the host
 *  wants a callback, hand back the reason. */
function fail(
  opts: PushRegisterOptions,
  reason: PushRegisterFailure,
  message: string,
): PushRegisterResult {
  logger.warn('push', `register failed (${reason}):`, message)
  // `register()` never throws is the contract, and this is the one
  // path that hands the host an error to look at — so it is also the
  // one place where the host's own code could have broken it.
  safely('onError', () => opts.onError?.(new Error(message)))
  return { ok: false, message, reason }
}

/**
 * Send the registration again because the person changed.
 *
 * Only for a device that has already registered — the vendor token is
 * the evidence, the same rule the native SDKs use for a rotated
 * token. Nothing here throws and nothing here is awaited by the host:
 * `user()` is synchronous and stays that way.
 */
function reRegisterAfterIdentityChange(): void {
  const token = _lastNativeToken
  const cfg = tryGetRuntimeConfig()
  if (token == null || cfg == null) return
  const identity = JSON.stringify([currentUserKey() ?? null, currentUserTraits() ?? null])
  if (identity === _lastSentIdentity) return
  _lastSentIdentity = identity
  void registerWithServer(cfg, token, _lastOptions).catch((e: unknown) => {
    // A failed update leaves the row pointing at the previous person,
    // so the next change must be allowed to try again.
    _lastSentIdentity = null
    logger.warn('push', 'updating the device after a sign-in failed', e)
  })
}

/** Internal carrier so the helpers below can name which failure they
 *  are without every one of them returning a result object. */
class PushRegisterError extends Error {
  readonly reason: PushRegisterFailure
  constructor(reason: PushRegisterFailure, message: string) {
    super(message)
    this.name = 'PushRegisterError'
    this.reason = reason
  }
}

/**
 * Revoke the cached handle (DELETE /v1/push/devices/{ipt}) +
 * unregister locally. Idempotent — repeat calls are no-ops.
 */
export async function unregister(): Promise<void> {
  const cfg = tryGetRuntimeConfig()
  const ipt = _cachedIpt ?? (await readPersistedIpt())
  if (cfg && ipt) {
    try {
      await fetch(joinUrl(cfg.ingestUrl, `/v1/push/devices/${ipt}`), {
        method: 'DELETE',
        headers: { authorization: `Bearer ${cfg.token}` },
      })
    } catch (e) {
      logger.warn('push', 'unregister server delete failed', e)
    }
  }
  nativePushUnregister()
  onIdentityChange(undefined)
  _lastNativeToken = null
  _lastSentIdentity = null
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
      throw new PushRegisterError('no-transport', `OS registration failed: ${state.error}`)
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
  throw new PushRegisterError('token-timeout', `no device token within ${timeoutMs} ms`)
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
  // `kind`, not `provider` — the server's field name, which this
  // sent for a year as `provider` and got a 422 for every time.
  const body: Record<string, unknown> = {
    // Which installation this is. The server keys the row on it, so a
    // rotated token updates this device rather than creating a second
    // one under a new address.
    installId: await installId(),
    kind: isAndroid ? 'fcm' : 'apns',
    nativeToken,
    // The same salted identity hash every event carries, so the
    // dashboard can address this device by the user who hit an
    // issue. Absent until the host calls `sentori.user()`.
    userKey: currentUserKey(),
  }
  if (env != null) body.env = env
  // `metadata` was an advertised option that no line of this file
  // read: it never reached the body, and `RegisterBody` had no field
  // for it, while `device_tokens.metadata` sat at `'{}'` since the
  // table was created. An integrator who passed it had no way to find
  // out it went nowhere. (Insight asked whether it was stored; it was
  // not — 2026-08-11.)
  if (opts.metadata != null) body.metadata = opts.metadata
  // Attributes of the person rather than of the device, kept apart so
  // a build channel called "pro" cannot answer a send aimed at the pro
  // plan. Absent leaves whatever the row already had; `{}` clears it,
  // which is what signing out sends.
  const traits = currentUserTraits()
  if (traits != null) body.traits = traits
  const res = await fetch(joinUrl(cfg.ingestUrl, '/v1/push/devices'), {
    method: 'POST',
    headers: {
      authorization: `Bearer ${cfg.token}`,
      'content-type': 'application/json',
    },
    body: JSON.stringify(body),
  })
  if (!res.ok) {
    throw new PushRegisterError('server-rejected', `/v1/push/devices HTTP ${res.status}`)
  }
  // The handle is the device_tokens row id — a uuid, which is what
  // the revoke and send routes take. It was parsed here as an
  // `ipt_*` string that no server has ever returned.
  const json = (await res.json()) as { spToken?: string }
  if (typeof json.spToken !== 'string' || json.spToken.length === 0) {
    throw new PushRegisterError('server-rejected', 'server did not return a device token id')
  }
  return json.spToken
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
  // Nothing in a tick may reject. The caller is `void pumpOnce()` on
  // an interval, so a rejection here is an unhandled rejection in the
  // host's process — a red box in development, and on some setups
  // worse — for something the host is entitled to have fail quietly.
  try {
    const state = await pushDrainState()
    flushBuffered(state.notifications, state.taps)
    await reportRotationIfAny(state.token)
  } catch (e) {
    logger.warn('push', 'a drain tick failed — the next one still runs', e)
  }
}

/// Tell the server when the vendor has handed the app a different
/// token from the one it registered with.
///
/// Both native layers already route a rotation into the *native*
/// `SentoriPush.handleRotatedToken`, and that refuses to act on a
/// device the native side never registered — which, for a React
/// Native app, is every device: registration goes through JS and the
/// handle lives in AsyncStorage. So the rotation arrived nowhere, and
/// the server held a dead token until the host next called
/// `register()`. For an app that stays resident that is not a bounded
/// wait.
///
/// This is the drain loop that already runs at 1 Hz and already reads
/// the token; it threw it away. Foreground only, like the rest of the
/// loop — a rotation while backgrounded is reported on the next tick
/// after the app comes back, which is bounded.
async function reportRotationIfAny(token: string | undefined): Promise<void> {
  if (token == null || token === '' || token === _lastNativeToken) return
  // Only a device that has registered: a token arriving for one that
  // never did is not something to act on unasked, which is the rule
  // both native SDKs follow.
  if (_lastNativeToken == null) return
  const cfg = tryGetRuntimeConfig()
  if (cfg == null) return
  const previous = _lastNativeToken
  _lastNativeToken = token
  try {
    const ipt = await registerWithServer(cfg, token, _lastOptions)
    _cachedIpt = ipt
    void persistIpt(ipt)
  } catch (e) {
    // Put it back, or one failed report means the rotation is never
    // mentioned again and the device is unreachable for good.
    _lastNativeToken = previous
    logger.warn('push', 'reporting a rotated token failed', e)
  }
}

/// Run the host's own code without letting it reach back into ours.
///
/// A push that does not arrive is a product decision the host can live
/// with. An exception out of an SDK it merely opted into is not — and
/// the host's handlers are the sharp edge here: they are its code,
/// they run inside our loop, and JavaScript throws for a living. One
/// throwing handler used to take the rest of the batch with it, then
/// the tick, then every later tick as an unhandled rejection.
function safely(what: string, fn: () => void): void {
  try {
    fn()
  } catch (e) {
    logger.warn('push', `${what} threw — carrying on`, e)
  }
}

function flushBuffered(
  notifications: Array<Record<string, unknown>>,
  taps: Array<Record<string, unknown>>,
): void {
  for (const raw of notifications) {
    // v2.26 — Observability link-through (rule #4). If the server
    // injected `_sentori.msgId` in v2.25+, drop a `push` breadcrumb,
    // emit `sentori.push.received` track, and queue the ack.
    safely('autoCorrelate', () => autoCorrelate(raw, 'received'))
    safely('onMessage', () => _onMessage?.(coerceNotification(raw)))
  }
  for (const raw of taps) {
    safely('autoCorrelate', () => autoCorrelate(raw, 'opened'))
    safely('onTap', () => _onTap?.(raw.userInfo ?? raw))
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
  pushSignal('push', { body: undefined, msgId, opened: eventType === 'opened', provider, title })

  // Track event: reuses the existing SDK event pipeline. Two
  // distinct names so dashboards can separate delivery from open.
  const trackName = eventType === 'opened' ? 'sentori.push.opened' : 'sentori.push.received'
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
      await fetch(joinUrl(cfg.ingestUrl, `/v1/push/deliveries/${msgId}/ack`), {
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

/// This installation's id, minted on first use and kept after.
///
/// Falls back to a module-scoped value when there is no AsyncStorage,
/// which is the same fallback the handle uses: worse than persistent,
/// better than nothing, and it still keeps one launch's rotations
/// pointed at one row.
async function installId(): Promise<string> {
  if (_installId != null) return _installId
  const storage = await tryAsyncStorage()
  if (storage) {
    try {
      const seen = await storage.getItem(INSTALL_KEY)
      if (typeof seen === 'string' && seen.length > 0) {
        _installId = seen
        return seen
      }
    } catch {
      // An unreadable store is a store with nothing in it.
    }
  }
  const fresh = randomId()
  _installId = fresh
  if (storage) {
    try {
      await storage.setItem(INSTALL_KEY, fresh)
    } catch (e) {
      logger.warn('push', 'AsyncStorage.setItem failed', e)
    }
  }
  return fresh
}

/// A uuid, without requiring one to exist.
///
/// `crypto.randomUUID` is present on newer Hermes and absent on older
/// ones, and this must not be the line that decides whether push
/// works on an older engine.
function randomId(): string {
  const c = (globalThis as { crypto?: { randomUUID?: () => string } }).crypto
  if (typeof c?.randomUUID === 'function') return c.randomUUID()
  let out = ''
  for (let i = 0; i < 32; i++) out += Math.floor(Math.random() * 16).toString(16)
  return out
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

/** Test-only: drop the cached handle and stop both timers, so one
 *  test's successful registration does not leave an interval running
 *  into the next. Production code paths must not call this. */
export function __resetForTests(): void {
  _cachedIpt = null
  _installId = null
  _onMessage = undefined
  _onTap = undefined
  _ackQueue = []
  _lastNativeToken = null
  _lastOptions = {}
  _lastSentIdentity = null
  onIdentityChange(undefined)
  teardownBufferDrain()
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
