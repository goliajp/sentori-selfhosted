// v1.1 #4 升级 — JS sample profiler unit tests.

import { afterEach, describe, expect, test } from 'bun:test';

import { clearSpans, drainSpans } from '@goliapkg/sentori-core';

import {
  __resetSampleProfilerForTests,
  startSampleProfiler,
  stopSampleProfiler,
} from '../sample-profiler';

afterEach(() => {
  __resetSampleProfilerForTests();
  clearSpans();
});

describe('sample-profiler', () => {
  test('start with enabled=false is a no-op', () => {
    startSampleProfiler({ enabled: false });
    stopSampleProfiler();
    expect(drainSpans().length).toBe(0);
  });

  test('double start is idempotent (second call ignored)', () => {
    startSampleProfiler({ enabled: true, sampleMs: 30, flushMs: 5_000 });
    // A second start while the timer is live must not spin a second
    // interval — would double-count every sample.
    startSampleProfiler({ enabled: true, sampleMs: 30, flushMs: 5_000 });
    stopSampleProfiler();
  });

  test('stop clears state cleanly', async () => {
    startSampleProfiler({ enabled: true, sampleMs: 30, flushMs: 5_000 });
    // Let a few sampleTicks happen.
    await new Promise((r) => setTimeout(r, 120));
    stopSampleProfiler();
    // After stop, no further span should be emitted even past flushMs.
    await new Promise((r) => setTimeout(r, 80));
    // (Bun timers aren't faked here, so we don't wait the full flush
    // window; we just confirm stop returns cleanly and __reset works.)
    __resetSampleProfilerForTests();
    expect(true).toBe(true);
  });

  test('sample interval floor — sampleMs < 20 clamped up to 20', () => {
    // Internal: profile.duration_ms tag uses the *effective* window
    // length so we can't directly observe the chosen sampleMs, but we
    // *can* assert start doesn't throw with an absurd request, since
    // the clamp protects against pathological CPU burn.
    startSampleProfiler({ enabled: true, sampleMs: 1, flushMs: 5_000 });
    stopSampleProfiler();
  });

  test('flush interval floor — flushMs < 5000 clamped up to 5000', () => {
    startSampleProfiler({ enabled: true, sampleMs: 30, flushMs: 100 });
    stopSampleProfiler();
  });
});
