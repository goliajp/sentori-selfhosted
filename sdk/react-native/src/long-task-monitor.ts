// v0.9.6 #4 — JS thread long-task monitor.
//
// What it does: a setInterval(50ms) tick checks how much wall-clock
// passed vs the requested interval. Excess = the JS thread was busy
// doing something else for at least that much. When the excess
// crosses 200ms (a "long task" per Chrome's PerformanceObserver
// threshold) we emit a `sentori.longtask` span so the dashboard's
// trace waterfall shows where the JS thread stalled.
//
// What it does NOT do: capture the stack DURING the long task. JS
// is single-threaded — by the time our tick runs, the busy code is
// already gone. The span carries the duration + nearest navigation
// context (via active span) so triage still has a route to blame.
//
// Why this and not a real Hermes sampler: Hermes ships a sampling
// profiler but accessing it from JS requires RN-internal bridges
// that vary per RN minor. We could swizzle / vendor headers but
// that's an Insight-build-config burden. The long-task monitor
// gets ~80% of the practical value (find slow renders, expensive
// reducers, accidental sync work in render path) with zero native
// code + zero RN-version sensitivity.
//
// Pairs naturally with +S4 (pre-crash sentinel, RAF frame budget,
// fires at the slow-frame threshold) — long-task monitor fires
// further down the slowness scale.

import { startSpan } from '@goliapkg/sentori-core';

const TICK_INTERVAL_MS = 50;
const LONGTASK_THRESHOLD_MS = 200; // > 200ms blocking = a longtask
const MAX_EMITS_PER_MIN = 60;

let _timer: ReturnType<typeof setInterval> | null = null;
let _lastTick = 0;
let _emitWindowStart = 0;
let _emitsThisWindow = 0;

export type LongTaskMonitorOptions = {
  enabled: boolean;
  /** Threshold ms above which a tick lag becomes a longtask span.
   *  Default 200ms. Lower → noisier. */
  thresholdMs?: number;
};

export function startLongTaskMonitor(opts: LongTaskMonitorOptions): void {
  if (_timer !== null) return;
  if (!opts.enabled) return;
  const threshold = opts.thresholdMs ?? LONGTASK_THRESHOLD_MS;
  _lastTick = Date.now();
  _emitWindowStart = _lastTick;
  _emitsThisWindow = 0;
  _timer = setInterval(() => {
    tick(threshold);
  }, TICK_INTERVAL_MS);
  (_timer as unknown as { unref?: () => void }).unref?.();
}

export function stopLongTaskMonitor(): void {
  if (_timer !== null) {
    clearInterval(_timer);
    _timer = null;
  }
}

function tick(threshold: number): void {
  const now = Date.now();
  const elapsed = now - _lastTick;
  _lastTick = now;
  const lag = elapsed - TICK_INTERVAL_MS;
  if (lag <= threshold) return;

  // Rate-limit emits: at most MAX_EMITS_PER_MIN per minute so a
  // pathological scroll-block-storm doesn't generate 1000 spans.
  if (now - _emitWindowStart >= 60_000) {
    _emitWindowStart = now;
    _emitsThisWindow = 0;
  }
  if (_emitsThisWindow >= MAX_EMITS_PER_MIN) return;
  _emitsThisWindow += 1;

  const span = startSpan('sentori.longtask', {
    name: 'js.longtask',
    startNowMs: now - lag,
    tags: {
      'profile.kind': 'longtask',
      'profile.duration_ms': String(Math.round(lag)),
      'profile.tick_interval_ms': String(TICK_INTERVAL_MS),
    },
  });
  span.finish({ endNowMs: now, status: 'ok' });
}

/** Test-only. */
export function __resetLongTaskMonitorForTests(): void {
  stopLongTaskMonitor();
  _lastTick = 0;
  _emitWindowStart = 0;
  _emitsThisWindow = 0;
}
