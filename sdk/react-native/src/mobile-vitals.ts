// v0.9.4 #1 — Mobile Vitals.
//
// Three measurements, three call paths, one server schema:
//
//   • Cold start: native side measures at app launch (iOS
//     mach_absolute_time / Android Process.getStartElapsedRealtime)
//     and exposes via the bundled native module — read once on JS
//     side at init() and ride along on the first event.
//   • TTID (Time-To-Initial-Display): automatic via
//     useTraceNavigation extension — span from navigation.dispatch
//     to first frame after route mount.
//   • TTFD (Time-To-Full-Display): manual. Host calls
//     sentori.markTimeToFullDisplay('Home').end() when the screen's
//     data has loaded.
//   • Slow / frozen frame counts: native side hooks CADisplayLink /
//     Choreographer.FrameCallback; counters flush per navigation
//     span.
//
// JS-side first: TTFD API + bundle-level vital captures. Native
// pieces ship as a separate native module method `getColdStartMs`
// + `getFrameCounts()` — graceful no-op if not linked.

import { startSpan } from '@goliapkg/sentori-core';

import { getNativeColdStartMs, getNativeFrameCounters } from './native';

let _coldStartMs: null | number = null;
let _coldStartCaptured = false;

/** Read the native-side cold start measurement once. Cached. Returns
 *  null when the native module isn't linked (Expo Go / tests). */
export function getColdStartMs(): null | number {
  if (_coldStartCaptured) return _coldStartMs;
  _coldStartCaptured = true;
  try {
    _coldStartMs = getNativeColdStartMs();
  } catch {
    _coldStartMs = null;
  }
  return _coldStartMs;
}

/**
 * v0.9.4 #1 — Time-To-Full-Display marker. Host calls this at the
 * point the screen is functionally "ready" (data fetched, images
 * loaded, etc.) so the dashboard can show real perceived load time
 * vs auto-detected TTID.
 *
 *     const h = sentori.markTimeToFullDisplay('Home');
 *     // ...data fetched, images rendered...
 *     h.end();
 *
 * If `.end()` is never called, the handle's span is finished as
 * `cancelled` at the next `markTimeToFullDisplay` call (one TTFD in
 * flight at a time is the typical case).
 */
export type TimeToFullDisplayHandle = {
  end: (opts?: { status?: 'cancelled' | 'error' | 'ok' }) => void;
  cancel: () => void;
};

let _activeTtfd: null | {
  finish: (status: 'cancelled' | 'error' | 'ok') => void;
  route: string;
} = null;

export function markTimeToFullDisplay(route: string): TimeToFullDisplayHandle {
  if (_activeTtfd && _activeTtfd.route !== route) {
    _activeTtfd.finish('cancelled');
  }
  const span = startSpan('react.navigation.ttfd', {
    name: route,
    tags: { 'nav.route': route, 'vital.kind': 'ttfd' },
  });
  let finished = false;
  const finish = (status: 'cancelled' | 'error' | 'ok'): void => {
    if (finished) return;
    finished = true;
    span.finish({ status });
    if (_activeTtfd && _activeTtfd.route === route) _activeTtfd = null;
  };
  _activeTtfd = { finish, route };
  return {
    cancel: () => finish('cancelled'),
    end: (opts?: { status?: 'cancelled' | 'error' | 'ok' }) =>
      finish(opts?.status ?? 'ok'),
  };
}

/** v0.9.4 #1 — read the per-screen slow/frozen frame counts since
 *  the most recent navigation transition. Native module reads
 *  CADisplayLink / Choreographer counters; returns null when not
 *  linked. */
export function getFrameCounters(): null | { slow: number; frozen: number } {
  try {
    return getNativeFrameCounters();
  } catch {
    return null;
  }
}

/** Test-only. */
export function __resetMobileVitalsForTests(): void {
  if (_activeTtfd) _activeTtfd.finish('cancelled');
  _activeTtfd = null;
  _coldStartCaptured = false;
  _coldStartMs = null;
}
