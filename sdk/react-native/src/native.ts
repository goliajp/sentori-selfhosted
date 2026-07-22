/**
 * Bridge to the native (iOS / Android) Sentori module.
 * No-op when not running in an Expo runtime that has the module installed —
 * this keeps the SDK usable in pure-JS environments (jest, bun test, web).
 */

import { logger } from '@goliapkg/sentori-core'

declare const __DEV__: boolean | undefined

type SentoriNativeModule = {
  drainPending: () => Promise<string[]>
  setConfig: (config: {
    environment: string
    release: string
    token: string
  }) => void
  /**
   * v0.9.4 #1 — cold start measurement. iOS:
   * `mach_absolute_time` from `applicationDidFinishLaunching` to first
   * JS bridge ready. Android: `Process.getStartElapsedRealtime()`.
   * Returns null when native side hasn't captured yet.
   */
  getColdStartMs?: () => null | number
  /**
   * v0.9.4 #1 — call once at JS init() to finalize the cold-start
   * measurement. iOS subtracts from the app-delegate anchor;
   * Android uses Process.getStartElapsedRealtime() so the call is
   * idempotent if missed.
   */
  markJsBridgeReady?: () => void
  /**
   * v0.9.4 #1 — slow/frozen frame counters since the most recent
   * navigation transition. Native side hooks `CADisplayLink` (iOS)
   * / `Choreographer.FrameCallback` (Android). Frame > 16.67ms =
   * slow; > 700ms = frozen.
   */
  getFrameCounters?: () => null | { frozen: number; slow: number }
  /** Reset counters on navigation transition (called by useTraceNavigation). */
  resetFrameCounters?: () => void
  /**
   * v0.9.5 #8 — read the most-recent native exception recorded by
   * `SentoriNativeExceptionBridge` within the last 1 s. Used by the
   * JS-side capture path to attach native stack info to a JSError
   * that RN wrapped from a swallowed NSException / Java Exception.
   */
  getRecentNativeException?: () => null | {
    ageMs: number
    name: string
    reason: string
    stack: string[]
  }
  /**
   * v0.7.3 — JS-triggered screenshot with consumer-supplied mask IDs.
   * `maskedIds` are RN `nativeID` strings; native walks the view
   * tree, finds each subview by identifier, and paints a black
   * rectangle over its frame in the captured bitmap. Resolves to
   * `null` if there's no key window / API < 24 (Android) / render
   * timed out. Replaced the previous `react-native-view-shot`
   * peer-dep path.
   */
  captureScreenshotWithMask?: (
    maskedIds: string[],
  ) => Promise<null | { base64: string; mediaType: string }>
  /**
   * v0.9.6 #2 — wireframe view-tree snapshot. iOS walks the
   * UIView hierarchy, paints each node as a rect/text descriptor,
   * intersects with masked nativeIDs. Returns an NDJSON-shaped
   * snapshot string or null on failure.
   */
  captureWireframe?: (maskedIds: string[]) => null | string
  /**
   * v0.9.12 — diagnostic readout for the wireframe path. Cheap
   * synchronous call that returns the path the last `captureWireframe`
   * tick took plus scene/window counts at that moment. Used by the
   * example app's debug button and the Insight verify flow to tell
   * "no window resolvable" from "window walked but tree empty" without
   * shipping a new pod.
   */
  probeWireframe?: () => {
    lastPath: string
    lastNodes: number
    sceneCount?: number
    windowCount?: number
    trackedSource?: string
    trackedActivity?: string
    decorViewFound?: boolean
    /** v1.0.0-rc.3: deepest descendant reached by the walker. If
     *  this is 2-3 the walker bailed early; if it matches the
     *  expected view-tree depth (~20-40 for RN apps) the capture
     *  is healthy. */
    lastDepthMax?: number
    /** v1.0.0-rc.3: byte length of the last serialised payload. */
    lastSizeBytes?: number
    /** v1.0.0-rc.3: lifetime counters — totalTicks is the number
     *  of times native captureWireframe ran; totalEmptyResultTicks
     *  is the subset that came back null or empty. Ratio surfaces
     *  intermittent failures (e.g. 200/240 ≈ 80% failure rate). */
    totalTicks?: number
    totalEmptyResultTicks?: number
  }
  /**
   * v1.0.0-rc.2 — diagnostic mirror of probeWireframe for the
   * screenshot path. Returned shape is best-effort cross-platform —
   * Android carries `trackedActivity` / `decorViewFound` / dims,
   * iOS carries `windowFound` / `rootViewControllerFound` / `bounds*`.
   * Callers should treat unknown keys as missing.
   */
  probeScreenshot?: () => Record<string, unknown>
  /**
   * Phase 22 sub-D / sub-E: cross-platform main-thread watchdog.
   * Android: 5 s / 1 s defaults (matches the OS ANR threshold).
   * iOS: 2 s / 1 s (more aggressive — iOS has no system-level
   * watchdog signal we can lean on, so we surface stutter Apple's
   * own runtime never flags).
   * Reports a `kind = "anr"` event when the main thread is wedged.
   */
  startAnrWatchdog?: (options?: {
    force?: boolean
    intervalMs?: number
    timeoutMs?: number
  }) => void
  stopAnrWatchdog?: () => void
  /** Dev-only — example app uses this to verify the crash flow. */
  triggerTestNativeCrash?: () => void
  /**
   * v2.9 — current iOS push permission status without prompting.
   * Returns `granted` / `denied` / `notDetermined` / `provisional`
   * / `ephemeral` (iOS-specific) mirroring web `Notification.permission`.
   */
  pushGetStatus?: () => Promise<string>
  /**
   * v2.9 — triggers the OS permission prompt the first time, or
   * returns the cached decision. Same return shape as
   * `pushGetStatus`.
   */
  pushRequestPermission?: () => Promise<string>
  /**
   * v2.9 — kicks off `UIApplication.registerForRemoteNotifications`.
   * The token comes back asynchronously via the AppDelegate swizzle
   * and lands in the native buffer — JS retrieves it through
   * `pushDrainState`.
   */
  pushRegister?: () => void
  /**
   * v2.9 — counterpart to `pushRegister`. Calls
   * `UIApplication.unregisterForRemoteNotifications` + clears the
   * cached token. The server-side DELETE is the JS layer's
   * responsibility.
   */
  pushUnregister?: () => void
  /**
   * v2.9 — snapshot the buffered token, foreground notifications,
   * and tap responses, and clear the buffers atomically. Returns
   *   { token?: string, error?: string, notifications: [...], taps: [...] }
   * where each notification carries
   *   { id, title, body, subtitle?, category?, userInfo, receivedAt }.
   */
  pushDrainState?: () => Promise<{
    token?: string
    error?: string
    notifications: Array<Record<string, unknown>>
    taps: Array<Record<string, unknown>>
  }>
}

let _native: SentoriNativeModule | null | undefined

function native(): SentoriNativeModule | null {
  if (_native !== undefined) return _native
  try {
    const core = require('expo-modules-core') as {
      requireNativeModule: <T>(name: string) => T
    }
    _native = core.requireNativeModule<SentoriNativeModule>('Sentori')
    if (typeof __DEV__ !== 'undefined' && __DEV__ && _native !== null) {
      // v0.9.9 — Insight asked for "tell me exactly which functions
      // the iOS pod is currently exposing." Logged once per process
      // (the cached _native short-circuits subsequent calls). Helps
      // distinguish "pod is stale" from "method exists but throws
      // at runtime" in one log line.
      const keys = Object.keys(_native as object).sort()
      logger.debug('native', 'module bound; methods:', keys.join(', ') || '(none)')
    }
  } catch (e) {
    _native = null
    logger.error('native', 'requireNativeModule("Sentori") threw', e)
  }
  return _native
}

export function setNativeConfig(config: {
  environment: string
  release: string
  token: string
}): void {
  try {
    native()?.setConfig(config)
  } catch {
    // never throw on init
  }
}

export async function drainNativePending(): Promise<string[]> {
  const n = native()
  if (!n) return []
  try {
    return await n.drainPending()
  } catch {
    return []
  }
}

/**
 * Dev-only helper. Triggers a real NSException (iOS) or RuntimeException
 * (Android) after a short delay so the host app crashes for real and the
 * native crash handler exercises the full write-to-disk path.
 *
 * Usage: tap a button in the example app, watch the app close, restart it,
 * verify the server received the event.
 *
 * No-op when the native module isn't installed (jest, bun test, web).
 */
export function triggerNativeCrash(): void {
  try {
    native()?.triggerTestNativeCrash?.()
  } catch {
    // never throw from a debugging helper
  }
}

/**
 * Phase 22 sub-D / sub-E: cross-platform main-thread watchdog.
 * Single JS call covers both Android ANR and iOS hang detection.
 *
 *     startAnrWatchdog()                       // platform defaults, prod-only
 *     startAnrWatchdog({ force: true })        // include debug builds
 *     startAnrWatchdog({ timeoutMs: 3000 })    // tighter threshold
 *
 * Defaults: Android 5 s / 1 s tick; iOS 2 s / 1 s tick. Returns
 * silently on web / jest / unsupported runtimes.
 */
export function startAnrWatchdog(options?: {
  force?: boolean
  intervalMs?: number
  timeoutMs?: number
}): void {
  try {
    native()?.startAnrWatchdog?.(options)
  } catch {
    // never throw from init helpers
  }
}

// ── v2.9 push wrappers ──────────────────────────────────────────

/** Returns the current iOS push permission status without
 *  prompting. `null` when the native module isn't available. */
export async function pushGetStatus(): Promise<null | string> {
  const n = native()
  if (!n?.pushGetStatus) return null
  try {
    return await n.pushGetStatus()
  } catch {
    return null
  }
}

/** Prompts for permission (or returns cached decision). `null` when
 *  the native module isn't available. */
export async function pushRequestPermission(): Promise<null | string> {
  const n = native()
  if (!n?.pushRequestPermission) return null
  try {
    return await n.pushRequestPermission()
  } catch {
    return null
  }
}

export function pushRegister(): void {
  try {
    native()?.pushRegister?.()
  } catch {
    /* never throw */
  }
}

export function pushUnregister(): void {
  try {
    native()?.pushUnregister?.()
  } catch {
    /* never throw */
  }
}

export type PushDrainState = {
  token?: string
  error?: string
  notifications: Array<Record<string, unknown>>
  taps: Array<Record<string, unknown>>
}

/** Snapshot + clear the native push buffers. Returns empty state
 *  when the native module isn't available. */
export async function pushDrainState(): Promise<PushDrainState> {
  const n = native()
  if (!n?.pushDrainState) {
    return { notifications: [], taps: [] }
  }
  try {
    return await n.pushDrainState()
  } catch {
    return { notifications: [], taps: [] }
  }
}

export function stopAnrWatchdog(): void {
  try {
    native()?.stopAnrWatchdog?.()
  } catch {
    // ignore
  }
}

/** v0.9.4 #1 — finalize cold-start measurement. Idempotent. */
export function markNativeJsBridgeReady(): void {
  try {
    native()?.markJsBridgeReady?.()
  } catch {
    // ignore
  }
}

/** v0.9.4 #1 — read cold start ms once. null when native unavailable. */
export function getNativeColdStartMs(): null | number {
  try {
    const v = native()?.getColdStartMs?.()
    return typeof v === 'number' && Number.isFinite(v) ? v : null
  } catch {
    return null
  }
}

/** v0.9.4 #1 — read slow/frozen frame counters since last reset. */
export function getNativeFrameCounters(): null | { frozen: number; slow: number } {
  try {
    return native()?.getFrameCounters?.() ?? null
  } catch {
    return null
  }
}

/** v0.9.4 #1 — reset frame counters on navigation transition. */
export function resetNativeFrameCounters(): void {
  try {
    native()?.resetFrameCounters?.()
  } catch {
    // ignore
  }
}

/** v0.9.5 #8 — fetch the most-recent native exception from
 *  SentoriNativeExceptionBridge (within last ~1 s). null if none or
 *  bridge not linked. */
export function getRecentNativeException(): null | {
  ageMs: number
  name: string
  reason: string
  stack: string[]
} {
  try {
    return native()?.getRecentNativeException?.() ?? null
  } catch {
    return null
  }
}

/**
 * v0.7.3 — drives the native screenshot path. JS side passes the
 * current list of mask `nativeID`s (read from the consumer's
 * registered mask query); native renders + redacts.
 *
 * Returns `null` on every failure mode: no native module bound
 * (jest, bun test, web), method missing (older native build still
 * deployed), capture failed (no key window, timed out, etc.).
 * Callers must treat `null` as "no screenshot this round" — the
 * error event still ships, just without a thumbnail.
 */
export async function captureNativeScreenshotWithMask(
  maskedIds: string[],
): Promise<null | { base64: string; mediaType: string }> {
  const n = native()
  if (!n) {
    logger.warn('native', 'module not bound — requireNativeModule("Sentori") threw')
    return null
  }
  if (!n.captureScreenshotWithMask) {
    logger.warn('native', 'captureScreenshotWithMask missing — pod install may be stale')
    return null
  }
  try {
    const r = await n.captureScreenshotWithMask(maskedIds)
    if (!r) {
      logger.debug('native', 'screenshot returned null — no key window / render failed')
    }
    return r
  } catch (e) {
    logger.warn('native', 'screenshot threw', e)
    return null
  }
}

/** v0.9.9 — diagnostic peek used by the wireframe replay tick. Same
 *  four-way distinction as captureNativeScreenshotWithMask. Logged at
 *  most once per `startReplay()` (the wrapper in replay.ts gates). */
export function describeWireframeNative(): {
  bound: boolean
  hasCaptureWireframe: boolean
  hasProbeWireframe: boolean
} {
  const n = native()
  return {
    bound: n !== null,
    hasCaptureWireframe: Boolean(n?.captureWireframe),
    hasProbeWireframe: Boolean(n?.probeWireframe),
  }
}

/**
 * v0.9.12 — JS entry to the native `probeWireframe` diagnostic. Safe
 * to call before the first replay tick — returns the not-yet-called
 * sentinel. When the ring stays empty, this is the single call that
 * answers "why" without redeploying the pod.
 *
 *   path             meaning
 *   ───────────────  ───────────────────────────────────────────────
 *   none(not-yet…)   captureWireframe has never run yet
 *   scene.fg.key     iOS: resolved via foregroundActive scene's key window
 *   scene.fg.first   iOS: foregroundActive scene's first window (no key)
 *   scene.fgi.first  iOS: foregroundInactive scene mid-transition
 *   scene.any.first  iOS: had to fall back to any window
 *   legacy.first     iOS: legacy UIApplication.windows path
 *   none             iOS: no UIWindow reachable at the tick instant
 *   activity.null    Android: no resumed Activity registered
 *   decorView.null   Android: activity has no decor view yet
 *   root.zero-size   Android: decorView size <= 0 (mid-layout)
 *   activity.resumed Android: ok
 */
export function probeNativeWireframe(): {
  available: boolean
  lastNodes: number
  lastPath: string
  sceneCount: number
  windowCount: number
  /** v1.0.0-rc.3: max recursion depth reached by the walker on
   *  the last tick. Healthy on an RN app: 20-40. If it's 2-3 the
   *  walker bailed early (zero-size parent, masked root). */
  lastDepthMax: number
  /** v1.0.0-rc.3: byte length of the last serialised payload. */
  lastSizeBytes: number
  /** v1.0.0-rc.3: lifetime totals. `totalEmptyResultTicks /
   *  totalTicks` is the failure rate. */
  totalTicks: number
  totalEmptyResultTicks: number
  raw: Record<string, unknown>
} {
  const n = native()
  if (!n || typeof n.probeWireframe !== 'function') {
    return {
      available: false,
      lastDepthMax: 0,
      lastNodes: 0,
      lastPath: 'native.unavailable',
      lastSizeBytes: 0,
      raw: {},
      sceneCount: 0,
      totalEmptyResultTicks: 0,
      totalTicks: 0,
      windowCount: 0,
    }
  }
  try {
    const r = n.probeWireframe()
    const raw = (r ?? {}) as Record<string, unknown>
    return {
      available: true,
      lastDepthMax: typeof r?.lastDepthMax === 'number' ? r.lastDepthMax : 0,
      lastNodes: typeof r?.lastNodes === 'number' ? r.lastNodes : 0,
      lastPath: typeof r?.lastPath === 'string' ? r.lastPath : 'unknown',
      lastSizeBytes: typeof r?.lastSizeBytes === 'number' ? r.lastSizeBytes : 0,
      raw,
      sceneCount: typeof r?.sceneCount === 'number' ? r.sceneCount : 0,
      totalEmptyResultTicks:
        typeof r?.totalEmptyResultTicks === 'number' ? r.totalEmptyResultTicks : 0,
      totalTicks: typeof r?.totalTicks === 'number' ? r.totalTicks : 0,
      windowCount: typeof r?.windowCount === 'number' ? r.windowCount : 0,
    }
  } catch (e) {
    logger.warn('native', 'probeWireframe threw', e)
    return {
      available: false,
      lastDepthMax: 0,
      lastNodes: 0,
      lastPath: 'native.threw',
      lastSizeBytes: 0,
      raw: {},
      sceneCount: 0,
      totalEmptyResultTicks: 0,
      totalTicks: 0,
      windowCount: 0,
    }
  }
}

/**
 * v1.0.0-rc.2 — JS entry to the `probeScreenshot` native diagnostic.
 * Same shape contract as [probeNativeWireframe]: returns a flat
 * key/value bag the consumer can ship back as-is when screenshot
 * capture returns null.
 *
 *   path                       meaning
 *   ─────────────────────────  ───────────────────────────────────────
 *   none(not-yet-called)       captureScreenshot has never run
 *   ok                         capture succeeded
 *   activity.null              Android: foreground tracker had no Activity
 *   window.null                Android/iOS: Activity/scene has no window
 *   decorView.null             Android: window had no decor view
 *   decorView.zero-size        Android: decorView size <= 0 (mid-layout)
 *   api.unsupported            Android: API < 24 (no PixelCopy)
 *   pixelCopy.notSuccess       Android: PixelCopy completed but reported failure
 *   pixelCopy.threw:<class>    Android: PixelCopy threw mid-request
 *   render.failed              iOS: UIGraphicsImageRenderer returned nil
 *   empty                      iOS: walked tree but no view+screenshot output
 *
 * On Android the result also carries `trackedSource` (lifecycle.created /
 * lifecycle.resumed / reflection.activityThread / manual.setActivity)
 * so callers can tell whether the SDK back-filled via reflection.
 */
export function probeNativeScreenshot(): {
  available: boolean
  lastPath: string
  raw: Record<string, unknown>
} {
  const n = native()
  if (!n || typeof n.probeScreenshot !== 'function') {
    return { available: false, lastPath: 'native.unavailable', raw: {} }
  }
  try {
    const r = n.probeScreenshot()
    const raw =
      r && typeof r === 'object' && !Array.isArray(r) ? (r as Record<string, unknown>) : {}
    const lastPath = typeof raw.lastPath === 'string' ? raw.lastPath : 'unknown'
    return { available: true, lastPath, raw }
  } catch (e) {
    logger.warn('native', 'probeScreenshot threw', e)
    return { available: false, lastPath: 'native.threw', raw: {} }
  }
}
