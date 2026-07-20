import { afterEach, beforeEach, expect, test } from 'bun:test';

import {
  __peekRuntimeMetricsSize,
  __resetRuntimeMetricsForTests,
  drainRuntimeMetricsForFlush,
} from '@goliapkg/sentori-core';

import {
  __forceEmitFpsForTests,
  __pushSampleForTests,
  __resetFpsInstrumentForTests,
  startFpsInstrument,
  stopFpsInstrument,
} from '../runtime-metrics-fps';
import {
  __forceHeapTickForTests,
  __peekHeapInstrumentState,
  startHeapInstrument,
  stopHeapInstrument,
} from '../runtime-metrics-heap';

beforeEach(() => {
  __resetRuntimeMetricsForTests();
  __resetFpsInstrumentForTests();
});

afterEach(() => {
  stopFpsInstrument();
  stopHeapInstrument();
});

test('fps: pushes p50 + p95 emits into the runtime metrics ring', () => {
  // 300 samples at a steady 16.67 ms cadence ≈ 60 fps.
  for (let i = 0; i < 300; i++) {
    __pushSampleForTests(16.67);
  }
  __forceEmitFpsForTests();

  const drained = drainRuntimeMetricsForFlush();
  expect(drained.length).toBe(2);
  const names = drained.map((m) => m.name).sort();
  expect(names).toEqual(['runtime.fps.p50', 'runtime.fps.p95']);
  // Both should land near 60 fps.
  const p50 = drained.find((m) => m.name === 'runtime.fps.p50')!;
  expect(p50.value).toBeGreaterThanOrEqual(58);
  expect(p50.value).toBeLessThanOrEqual(62);
});

test('fps: mixed-cadence run separates p50 from p95 sensibly', () => {
  // 270 fast frames (60 fps) + 30 stutters (30 fps). p95 should
  // land at the stutter zone.
  for (let i = 0; i < 270; i++) __pushSampleForTests(16.67);
  for (let i = 0; i < 30; i++) __pushSampleForTests(33.33);
  __forceEmitFpsForTests();

  const drained = drainRuntimeMetricsForFlush();
  const p50 = drained.find((m) => m.name === 'runtime.fps.p50')!.value;
  const p95 = drained.find((m) => m.name === 'runtime.fps.p95')!.value;
  // p50 stays at the smooth-frame zone.
  expect(p50).toBeGreaterThanOrEqual(58);
  // p95 (worst-95th of Δt → slowest 5%) should drop into the
  // 30 fps zone.
  expect(p95).toBeLessThanOrEqual(35);
});

test('fps: start is idempotent — second call is a no-op', () => {
  startFpsInstrument();
  // Calling start twice mustn't spin up a second rAF loop. We
  // don't directly observe that here (no two-loop probe), but
  // we assert start doesn't throw + state stays running once.
  expect(() => startFpsInstrument()).not.toThrow();
  stopFpsInstrument();
});

test('heap: start is a no-op on hosts without performance.memory', () => {
  // The default test runtime (bun) doesn't expose
  // performance.memory, so start should silently skip and
  // never schedule a timer.
  startHeapInstrument();
  expect(__peekHeapInstrumentState().running).toBe(false);
});

test('heap: force tick is a no-op on hosts without performance.memory', () => {
  __forceHeapTickForTests();
  // No metric should land in the ring.
  expect(__peekRuntimeMetricsSize()).toBe(0);
});

test('heap: emits when performance.memory is shimmed', () => {
  // Shim performance.memory so the read path exercises.
  const g = globalThis as {
    performance?: { memory?: { usedJSHeapSize?: number; totalJSHeapSize?: number } };
  };
  const before = g.performance;
  g.performance = {
    ...(before ?? {}),
    memory: { usedJSHeapSize: 12_345_678, totalJSHeapSize: 23_456_789 },
  };
  try {
    __forceHeapTickForTests();
    const drained = drainRuntimeMetricsForFlush();
    const names = drained.map((m) => m.name).sort();
    expect(names).toEqual(['runtime.heap.total_bytes', 'runtime.heap.used_bytes']);
    expect(drained.find((m) => m.name === 'runtime.heap.used_bytes')!.value).toBe(12_345_678);
  } finally {
    g.performance = before;
  }
});
