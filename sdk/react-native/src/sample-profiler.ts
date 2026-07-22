// v1.1 #4 升级 — JS sample profiler (idle-tick sampler).
//
// What it does:
//   • setInterval(50ms) — captures `new Error().stack` of the tick
//     callback, parses out the top frames, increments per-frame
//     counts in a rolling window
//   • every 60 s, emits a `sentori.profile` span with the aggregated
//     flame-data (frame name → tick count) as span data
//
// What it does NOT do:
//   • Sample frames during a busy JS task. JS is single-threaded;
//     while the busy code is running, our setInterval can't fire,
//     so the busy stack never gets sampled. Long-task monitor (#4
//     v0.9.6) catches that case from a different angle (duration).
//
// Idle-tick sampling is still useful: shows which functions appear
// most often between busy moments — typically the render hot path,
// frequent timer callbacks, recurring middleware. Pairs with
// long-task-monitor (catches the 200ms+ outliers).
//
// Real Hermes off-thread sampling profiler (which would catch busy
// frames too) needs RN-internal HermesAPI access, deferred to v1.2.

import { startSpan } from '@goliapkg/sentori-core';

const SAMPLE_INTERVAL_MS = 50;
const FLUSH_INTERVAL_MS = 60_000;
const MAX_FRAMES = 200; // safety cap per profile
const MAX_FRAME_NAME_LEN = 120;

/** Floor on user-configured sample interval. Going below ~20 ms costs
 *  measurable JS-thread time to `new Error().stack` + regex parse on
 *  every tick — past that point you'd see the profile drag down the
 *  thing you're profiling. */
const MIN_SAMPLE_INTERVAL_MS = 20;

/** Floor on flush interval. Sub-5s windows produce a span every few
 *  seconds, which is just noise for an aggregator that's looking at
 *  per-minute hotspot trends. */
const MIN_FLUSH_INTERVAL_MS = 5_000;

/** How many stack-trace lines to keep per tick (after dropping the
 *  Error ctor + sampleTick frames). 10 is enough to see the JS-side
 *  call shape; deeper than that the bottom frames are usually RN
 *  runtime / event loop and not actionable. */
const FRAMES_PER_TICK = 10;

let _frameCounts = new Map<string, number>();
let _windowStartedAt = 0;
let _sampleTimer: ReturnType<typeof setInterval> | null = null;
let _flushTimer: ReturnType<typeof setInterval> | null = null;

export type SampleProfilerOptions = {
  enabled: boolean;
  /** Sample interval ms. Default 50. Lower → more accurate but more
   *  JS-thread overhead. */
  sampleMs?: number;
  /** Flush window ms. Default 60 000 (one minute). */
  flushMs?: number;
};

/**
 * Start the idle-tick sample profiler. Idempotent — calling twice
 * is a no-op after the first successful start.
 *
 * Pairs naturally with `longTaskMonitor` (≥200ms outliers): the
 * profiler shows the *distribution* of code that runs in idle gaps,
 * the long-task monitor catches the few outliers that are blocking
 * the thread. Together they cover the JS-side perf signal cheaply.
 */
export function startSampleProfiler(opts: SampleProfilerOptions): void {
  if (!opts.enabled || _sampleTimer !== null) return;
  const sampleMs = Math.max(MIN_SAMPLE_INTERVAL_MS, opts.sampleMs ?? SAMPLE_INTERVAL_MS);
  const flushMs = Math.max(MIN_FLUSH_INTERVAL_MS, opts.flushMs ?? FLUSH_INTERVAL_MS);

  _windowStartedAt = Date.now();
  _sampleTimer = setInterval(() => sampleTick(), sampleMs);
  _flushTimer = setInterval(() => flushWindow(), flushMs);
  (_sampleTimer as unknown as { unref?: () => void }).unref?.();
  (_flushTimer as unknown as { unref?: () => void }).unref?.();
}

export function stopSampleProfiler(): void {
  if (_sampleTimer !== null) {
    clearInterval(_sampleTimer);
    _sampleTimer = null;
  }
  if (_flushTimer !== null) {
    clearInterval(_flushTimer);
    _flushTimer = null;
  }
  _frameCounts.clear();
  _windowStartedAt = 0;
}

function sampleTick(): void {
  const stack = new Error().stack;
  if (!stack) return;
  // Skip the first 2 frames — they're our `sampleTick` + `Error
  // ctor` which would dominate every sample.
  const lines = stack.split('\n').slice(2, 2 + FRAMES_PER_TICK);
  for (const line of lines) {
    const frame = parseFrameName(line);
    if (frame) {
      _frameCounts.set(frame, (_frameCounts.get(frame) ?? 0) + 1);
    }
  }
}

/** Pull a stable identifier from one stack-trace line. Handles:
 *
 *     at FunctionName (file://path/Foo.js:123:45)
 *     at file://path/Foo.js:123:45                   (anonymous)
 *     FunctionName@file://path/Foo.js:123:45         (Hermes style)
 *
 * Drops absolute path noise — keeps `Foo.js:Line` so re-deploys
 * with stable code paths bucket together. */
function parseFrameName(line: string): null | string {
  const trimmed = line.trim();
  if (trimmed.length === 0) return null;
  // Hermes / JSC: `FunctionName@file:line:col` or just `@file:line:col`.
  // Standard:    `at FunctionName (file:line:col)`.
  let raw = trimmed.replace(/^at\s+/, '');
  raw = raw.replace(/\s*[(\[].*$/, ''); // strip the file part after `(`
  // Hermes: split on @
  if (raw.includes('@')) {
    raw = raw.split('@')[0] ?? raw;
  }
  raw = raw.trim();
  if (raw.length === 0 || raw.length > MAX_FRAME_NAME_LEN) return null;
  // Ignore the obvious noise frames.
  if (raw === 'Object.<anonymous>' || raw === '<anonymous>') return null;
  return raw;
}

function flushWindow(): void {
  if (_frameCounts.size === 0) return;
  const windowEndedAt = Date.now();
  const durationMs = windowEndedAt - _windowStartedAt;
  // Top-N frames so attachment doesn't bloat unboundedly.
  const top = Array.from(_frameCounts.entries())
    .sort((a, b) => b[1] - a[1])
    .slice(0, MAX_FRAMES);

  const span = startSpan('sentori.profile', {
    name: 'js.sample-profile',
    startNowMs: _windowStartedAt,
    tags: {
      'profile.kind': 'sample',
      'profile.sample_count': String(sampleCount(top)),
      'profile.duration_ms': String(durationMs),
    },
  });
  span.setData('flameData', Object.fromEntries(top));
  span.finish({ endNowMs: windowEndedAt, status: 'ok' });

  _frameCounts.clear();
  _windowStartedAt = windowEndedAt;
}

function sampleCount(entries: [string, number][]): number {
  let total = 0;
  for (const [, n] of entries) total += n;
  return total;
}

export function __resetSampleProfilerForTests(): void {
  stopSampleProfiler();
}
