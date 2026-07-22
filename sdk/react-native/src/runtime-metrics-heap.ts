// v2.1 W2 part 3 — JS heap auto-instrument.
//
// Polls `performance.memory.usedJSHeapSize` every 30 s, emits
// runtime.heap.used_bytes. Web-only metric, RN best-effort —
// the V8/Hermes value depends on which engine RN is wired to:
//   • Hermes (RN ≥ 0.70 default): exposes a per-isolate count
//     when memoryReporter is enabled (Hermes 0.12+)
//   • JSC (legacy / explicit opt-in): no native equivalent
//   • Web (sdk-javascript wires this same module): Chromium
//     gates it behind a high-resolution-timer flag in some
//     contexts; treat missing field as a silent no-op
//
// Cost: one number read + one emit every 30 s. Negligible.
// The timer .unref()s so Node tests / CLI hosts exit cleanly.

import { emitMetric } from '@goliapkg/sentori-core';

const TICK_MS = 30_000;

type MemoryShape = {
  usedJSHeapSize?: number;
  totalJSHeapSize?: number;
  jsHeapSizeLimit?: number;
};

let _timer: null | ReturnType<typeof setInterval> = null;

function readHeap(): MemoryShape | null {
  const perf = (globalThis as { performance?: { memory?: MemoryShape } }).performance;
  return perf?.memory ?? null;
}

function tickOnce(): void {
  const m = readHeap();
  if (!m) return;
  if (typeof m.usedJSHeapSize === 'number' && Number.isFinite(m.usedJSHeapSize)) {
    emitMetric('runtime.heap.used_bytes', m.usedJSHeapSize);
  }
  // Total + limit help capacity-plan but cost an extra ~16 B
  // per row downstream — keep them only when present.
  if (typeof m.totalJSHeapSize === 'number' && Number.isFinite(m.totalJSHeapSize)) {
    emitMetric('runtime.heap.total_bytes', m.totalJSHeapSize);
  }
  if (typeof m.jsHeapSizeLimit === 'number' && Number.isFinite(m.jsHeapSizeLimit)) {
    emitMetric('runtime.heap.limit_bytes', m.jsHeapSizeLimit);
  }
}

/** Idempotent start. No-op on hosts that don't expose
 *  performance.memory — checked once at start so we don't pay
 *  for the detect on every tick. */
export function startHeapInstrument(): void {
  if (_timer !== null) return;
  if (!readHeap()) return;
  // One immediate sample at start so the first dashboard render
  // doesn't sit empty for 30 s.
  tickOnce();
  _timer = setInterval(tickOnce, TICK_MS);
  (_timer as unknown as { unref?: () => void }).unref?.();
}

/** Stop. Idempotent. */
export function stopHeapInstrument(): void {
  if (_timer !== null) {
    clearInterval(_timer);
    _timer = null;
  }
}

/** Test-only force tick (skips the 30 s wait). */
export function __forceHeapTickForTests(): void {
  tickOnce();
}

/** Test-only state. */
export function __peekHeapInstrumentState(): { running: boolean } {
  return { running: _timer !== null };
}
