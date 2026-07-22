// v1.1 chunk B — analytics `track` events on the SDK side.
//
// `sentori.track(name, props?)` pushes a single typed analytics event
// into a fixed-size ring. A timer flushes the ring every 30 s (or
// when the buffer hits 500) to `POST /v1/track:batch`. Best-effort;
// a flush failure drops the batch — analytics aren't critical
// telemetry.
//
// Why not one fetch per event: a busy app emitting `$pageview` +
// custom funnel events every nav would saturate the JS thread and
// the host's outbound connection pool. The same batching shape the
// metrics module uses keeps `track()` cheap to call from render
// hooks.
//
// Auto-pageview (`$pageview`) is emitted from `useTraceNavigation`
// in navigation.ts whenever react-navigation swaps the active route.
// Hosts that don't use react-navigation can call
// `sentori.track('$pageview', { route: 'Cart' })` themselves.

import { addInternalBreadcrumb } from './breadcrumbs';
import { getCurrentUserId } from './capture';
import { getConfig, isInitialized } from './config';
import { sendTrackBatch } from './transport';

export type TrackProps = Record<string, unknown>;

export type TrackEvent = {
  environment?: string;
  name: string;
  props?: TrackProps;
  release?: string;
  route?: string;
  sessionId?: string;
  ts: string;
  userId?: string;
};

const MAX_BUFFER = 500;
const NAME_MAX = 200;
const PROPS_KEYS_MAX = 40;
const FLUSH_INTERVAL_MS = 30_000;

let _buf: TrackEvent[] = [];
let _timer: null | ReturnType<typeof setInterval> = null;

/**
 * Record a typed analytics event. Cheap to call from render hooks —
 * pushes into a 500-slot ring drained every 30 s (or on overflow) by
 * the transport flusher.
 *
 * Reserved names start with `$` (e.g. `$pageview`) and are emitted by
 * the SDK itself; you can still call `track('$pageview', …)` from app
 * code to backfill routes the auto-instrumentation missed.
 *
 * Server caps: name ≤ 200 chars, ≤ 40 prop keys. Calls exceeding the
 * cap are dropped client-side (no throw) so app code can fire-and-
 * forget without try/catch.
 */
export function track(name: string, props?: TrackProps, route?: string): void {
  if (!isInitialized()) return;
  if (typeof name !== 'string' || name.length === 0 || name.length > NAME_MAX) {
    return;
  }
  if (props && Object.keys(props).length > PROPS_KEYS_MAX) {
    return;
  }
  const config = getConfig();
  const ev: TrackEvent = {
    name,
    props,
    release: config?.release,
    environment: config?.environment,
    route,
    ts: new Date().toISOString(),
    userId: getCurrentUserId(),
  };
  _buf.push(ev);
  // v2.0 W3 — auto-breadcrumb. When `init.capture.trackAutoBreadcrumb`
  // is `true`, push a `{ type: 'track', data: { name, props } }`
  // breadcrumb so the customer journey leading up to a later
  // `captureException` / `captureMessage` is visible in the dashboard.
  // Defaults off — see Config.trackAutoBreadcrumb docstring.
  if (config?.trackAutoBreadcrumb === true) {
    addInternalBreadcrumb('track', props ? { name, props } : { name });
  }
  if (_buf.length >= MAX_BUFFER) {
    void flushTrack();
  }
}

export async function flushTrack(): Promise<void> {
  if (_buf.length === 0) return;
  const config = getConfig();
  if (!config) return;
  const batch = _buf;
  _buf = [];
  await sendTrackBatch(config.ingestUrl, config.token, batch);
}

/**
 * Start the 30 s flush timer. Called once from `init()`. Idempotent.
 * `__resetTrackForTests` is exposed for vitest / bun:test teardown.
 */
export function startTrackTimer(): void {
  if (_timer !== null) return;
  _timer = setInterval(() => {
    void flushTrack();
  }, FLUSH_INTERVAL_MS);
  // Don't keep the process alive solely for this timer.
  (_timer as unknown as { unref?: () => void }).unref?.();
}

export function __peekTrackBuffer(): readonly TrackEvent[] {
  return _buf;
}

export function __resetTrackForTests(): void {
  if (_timer !== null) {
    clearInterval(_timer);
    _timer = null;
  }
  _buf = [];
}
