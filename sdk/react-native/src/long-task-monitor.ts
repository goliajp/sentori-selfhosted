// long_freeze — detected warn scenario (category B:
// 「app 卡住了 / 死了几秒」), JS-thread side.
//
// Mini-spec: a setInterval(50 ms) tick measures monotonic-clock drift while active.
// Drift ≤ 2 s ⇒ a `freeze` signal in the ring (context only —
// brief jank is not worth an event). Drift > 2 s ⇒ one `warn`
// event, scenario `long_freeze`, surface = current screen, with the
// blocked duration. Rate-limited to 6 events/minute so a freeze
// storm cannot flood ingest.
//
// What it cannot do: capture the stack DURING the block — JS is
// single-threaded; by the time our tick runs the busy code is gone.
// Nor can it see the UI thread: a frozen UI with a healthy JS thread
// is invisible here. The native watchdogs (HangWatchdog / AnrWatchdog)
// cover that side, and are opt-in via `detect.uiThreadHang` — this
// comment used to say they covered it, full stop, while nothing in JS
// had ever started them.

import { pushSignal } from '@goliapkg/sentori-core';

import { appLifecycle } from './handlers/lifecycle';
import { currentScreen } from './navigation';
import { warnDetected } from './verbs';

const TICK_INTERVAL_MS = 50;
const SIGNAL_THRESHOLD_MS = 200; // ring entry: noticeable jank
const WARN_THRESHOLD_MS = 2_000; // warn event: a real freeze
const MAX_WARNS_PER_MIN = 6;

let _timer: ReturnType<typeof setInterval> | null = null;
let _lastTick = 0;
let _epoch = 0;
let _active = true;
const monotonicNow = (): number => globalThis.performance?.now?.() ?? Date.now();
let _warnWindowStart = 0;
let _warnsThisWindow = 0;

export function startLongTaskMonitor(): void {
  if (_timer !== null) return;
  _lastTick = monotonicNow();
  const lifecycle = appLifecycle();
  _epoch = lifecycle.epoch;
  _active = lifecycle.active;
  _warnWindowStart = _lastTick;
  _warnsThisWindow = 0;
  _timer = setInterval(() => {
    try { tick(); } catch { /* Instrumentation must not crash the host. */ }
  }, TICK_INTERVAL_MS);
  (_timer as unknown as { unref?: () => void }).unref?.();
}

export function stopLongTaskMonitor(): void {
  if (_timer !== null) {
    clearInterval(_timer);
    _timer = null;
  }
}

function tick(): void {
  const now = monotonicNow();
  const lifecycle = appLifecycle();
  const interrupted = !lifecycle.active || !_active || lifecycle.epoch !== _epoch;
  _epoch = lifecycle.epoch;
  _active = lifecycle.active;
  const elapsed = now - _lastTick;
  _lastTick = now;
  if (interrupted || elapsed < 0) return;
  const lag = elapsed - TICK_INTERVAL_MS;
  if (lag <= SIGNAL_THRESHOLD_MS) return;

  pushSignal('freeze', { blockedMs: Math.round(lag) });
  if (lag <= WARN_THRESHOLD_MS) return;

  if (now - _warnWindowStart >= 60_000) {
    _warnWindowStart = now;
    _warnsThisWindow = 0;
  }
  if (_warnsThisWindow >= MAX_WARNS_PER_MIN) return;
  _warnsThisWindow += 1;

  warnDetected(
    'long_freeze',
    { screen: currentScreen() },
    { blockedMs: Math.round(lag), thread: 'js' },
  );
}

/** Test-only. */
export function __resetLongTaskMonitorForTests(): void {
  stopLongTaskMonitor();
  _lastTick = 0;
  _warnWindowStart = 0;
  _warnsThisWindow = 0;
}
