// slow_cold_start — detected warn scenario (design.md §3, category
// C: 「启动过慢」).
//
// Mini-spec: the native side measures launch → JS-ready (iOS
// mach_absolute_time / Android Process.getStartElapsedRealtime);
// init() calls checkColdStart() once. > 3 s ⇒ one `warn` event,
// scenario `slow_cold_start`, with the measured ms. Always pushes a
// `lifecycle` signal so even a fast start shows on the timeline.
//
// Pre-warmed processes (iOS ActivePrewarm; Android background start
// — FCM data message, JobScheduler — with the first Activity
// arriving much later) produce phantom samples: the measured span
// is mostly idle background time, not launch. Those samples still
// ship, flagged `prewarmed: true`, so the field data stays honest
// and slicable — but they never fire the slow-start warn (insight
// QIP-8 #1; the Firebase Performance bug class).
//
// Graceful no-op when the native module isn't linked (Expo Go,
// tests).

import { pushSignal } from '@goliapkg/sentori-core';

import { getNativeColdStartMs, getNativeColdStartPrewarmed } from './native';
import { warnDetected } from './verbs';

const SLOW_COLD_START_MS = 3_000;

let _coldStartMs: null | number = null;
let _coldStartPrewarmed = false;
let _coldStartCaptured = false;

/** Read the native-side cold start measurement once. Cached. */
export function getColdStartMs(): null | number {
  if (_coldStartCaptured) return _coldStartMs;
  _coldStartCaptured = true;
  try {
    _coldStartMs = getNativeColdStartMs();
    _coldStartPrewarmed = getNativeColdStartPrewarmed();
  } catch {
    _coldStartMs = null;
  }
  return _coldStartMs;
}

/** Whether the cached measurement is a pre-warmed phantom. Only
 *  meaningful after getColdStartMs() ran. */
export function isColdStartPrewarmed(): boolean {
  return _coldStartPrewarmed;
}

/** Called once from init(): signal always, warn when slow AND real. */
export function checkColdStart(detectEnabled: boolean): void {
  const ms = getColdStartMs();
  if (ms === null) return;
  const prewarmed = isColdStartPrewarmed();
  pushSignal('lifecycle', {
    phase: 'cold_start',
    ms,
    ...(prewarmed ? { prewarmed: true } : {}),
  });
  if (detectEnabled && !prewarmed && ms > SLOW_COLD_START_MS) {
    warnDetected('slow_cold_start', {}, { coldStartMs: ms, thresholdMs: SLOW_COLD_START_MS });
  }
}

/** Test-only. */
export function __resetMobileVitalsForTests(): void {
  _coldStartCaptured = false;
  _coldStartMs = null;
  _coldStartPrewarmed = false;
}
