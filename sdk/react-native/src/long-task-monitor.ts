// long_freeze — detected warn scenario (design.md §3, category B:
// 「app 卡住了 / 死了几秒」), JS-thread side.
//
// Mini-spec: a setInterval(50 ms) tick measures wall-clock drift.
// Drift ≤ 2 s ⇒ a `freeze` signal in the ring (context only —
// brief jank is not worth an event). Drift > 2 s ⇒ one `warn`
// event, scenario `long_freeze`, surface = current screen, with the
// blocked duration. Rate-limited to 6 events/minute so a freeze
// storm cannot flood ingest.
//
// What it cannot do: capture the stack DURING the block — JS is
// single-threaded; by the time our tick runs the busy code is gone.
// The native watchdogs (HangWatchdog / AnrWatchdog) cover the
// UI-thread side of the same scenario.

import { pushSignal } from '@goliapkg/sentori-core';

import { currentScreen } from './navigation';
import { warnDetected } from './verbs';

const TICK_INTERVAL_MS = 50;
const SIGNAL_THRESHOLD_MS = 200; // ring entry: noticeable jank
const WARN_THRESHOLD_MS = 2_000; // warn event: a real freeze
const MAX_WARNS_PER_MIN = 6;

let _timer: ReturnType<typeof setInterval> | null = null;
let _lastTick = 0;
let _warnWindowStart = 0;
let _warnsThisWindow = 0;

export function startLongTaskMonitor(): void {
  if (_timer !== null) return;
  _lastTick = Date.now();
  _warnWindowStart = _lastTick;
  _warnsThisWindow = 0;
  _timer = setInterval(tick, TICK_INTERVAL_MS);
  (_timer as unknown as { unref?: () => void }).unref?.();
}

export function stopLongTaskMonitor(): void {
  if (_timer !== null) {
    clearInterval(_timer);
    _timer = null;
  }
}

function tick(): void {
  const now = Date.now();
  const elapsed = now - _lastTick;
  _lastTick = now;
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
